//! Scale benchmark: one program template instantiated per equipment.
//!
//! The scenario: a building-automation site where every piece of
//! equipment (here: VAV boxes) runs its own instance of the same
//! program, bound to that equipment's points through a connector. All
//! the connectors share one point source, which pushes point changes
//! in and receives the values the programs publish.
//!
//! ```text
//!  feeder thread (simulated point changes)
//!        │ push(equip, point, value)            ▲ publish(equip, point, value)
//!        ▼                                      │
//!  ┌──────────── shared Backend (one) ──────────┴───┐
//!  │  subscriptions by (equip, point)              │
//!  └──▲───────────────▲───────────────────▲────────┘
//!     │ ScopedConnector "db:0"   "db:1"    …  "db:N-1"
//!  engine 0         engine 1         …   engine N-1   (one tokio MT runtime)
//!  VAV template     VAV template         VAV template
//! ```
//!
//! Every engine loads the *same* template program; only the
//! `connector` pin is rewritten per instance (`db` → `db:<equip>`).
//! Addresses stay relative to the equipment (`zoneTemp`, `damperCmd`,
//! …) and the scoped connector resolves them in its equipment's
//! context.
//!
//! Latency is measured on a probe path inside each instance —
//! `ExternalIn(probe) → Add(+0) → ExternalOut(probeOut)` — whose value
//! is the push timestamp, so the publish handler can compute
//! push-to-publish latency exactly.
//!
//! Phases: start-up (load N engines, wait for every subscription),
//! load (BAS-like rates), idle (no point changes; the PIDs still tick
//! on the values they have), stress (rates ×20), shutdown.
//!
//! Run (one size per process, so memory numbers are not polluted by
//! earlier runs):
//!
//! ```bash
//! cargo run --release --example scale_bench --features multi-threaded -- <engines> [workers]
//! ```
//!
//! Phase lengths (seconds) can be overridden with `BENCH_IDLE`,
//! `BENCH_LOAD` and `BENCH_STRESS`, and each engine's scan interval
//! (milliseconds, default 200) with `BENCH_SCAN`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedSender, unbounded};

use logic_mesh::base::connector::{Connector, ConnectorFuture, ConnectorHandle, ValueStream};
use logic_mesh::base::program::{Program, ProgramBuilder};
use logic_mesh::multi_threaded::MultiThreadedEngine;
use logic_mesh::prelude::*;
use logic_mesh::{ConnectorError, Value};

// ---------------------------------------------------------------------
// Points
// ---------------------------------------------------------------------

/// The points the template subscribes to, per equipment.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Point {
    ZoneTemp,
    ZoneSp,
    Dat,
    Occ,
    Probe,
}

impl Point {
    const DATA: [Point; 4] = [Point::ZoneTemp, Point::ZoneSp, Point::Dat, Point::Occ];

    fn parse(address: &str) -> Option<Self> {
        Some(match address {
            "zoneTemp" => Point::ZoneTemp,
            "zoneSp" => Point::ZoneSp,
            "dat" => Point::Dat,
            "occ" => Point::Occ,
            "probe" => Point::Probe,
            _ => return None,
        })
    }
}

/// Subscriptions per instance: one per `ExternalIn` in the template.
const SUBS_PER_INSTANCE: usize = 5;

// ---------------------------------------------------------------------
// Shared backend + per-equipment scoped connector
// ---------------------------------------------------------------------

type Sender = UnboundedSender<Result<Value, ConnectorError>>;

/// The shared point source: one subscription table for every
/// equipment, and the sink for every publish.
struct Backend {
    epoch: Instant,
    subs: Mutex<HashMap<(u32, Point), Vec<Sender>>>,
    pushes: AtomicU64,
    publishes: AtomicU64,
    recording: AtomicBool,
    /// Push-to-publish latencies of the probe path, in microseconds.
    latencies: Mutex<Vec<u32>>,
}

impl Backend {
    fn new() -> Self {
        Self {
            epoch: Instant::now(),
            subs: Mutex::default(),
            pushes: AtomicU64::new(0),
            publishes: AtomicU64::new(0),
            recording: AtomicBool::new(false),
            latencies: Mutex::default(),
        }
    }

    fn now_nanos(&self) -> f64 {
        self.epoch.elapsed().as_nanos() as f64
    }

    fn live_subscriptions(&self) -> usize {
        let mut subs = self.subs.lock().unwrap();
        subs.values_mut()
            .map(|v| {
                v.retain(|tx| !tx.is_closed());
                v.len()
            })
            .sum()
    }

    /// Fans a point change out to every subscription of that point.
    /// Callable from any thread.
    fn push(&self, equip: u32, point: Point, value: Value) {
        let mut subs = self.subs.lock().unwrap();
        if let Some(list) = subs.get_mut(&(equip, point)) {
            list.retain(|tx| tx.unbounded_send(Ok(value.clone())).is_ok());
        }
        self.pushes.fetch_add(1, Ordering::Relaxed);
    }

    fn on_publish(&self, address: &str, value: &Value) {
        self.publishes.fetch_add(1, Ordering::Relaxed);
        if address == "probeOut"
            && self.recording.load(Ordering::Relaxed)
            && let Ok(sent) = TryInto::<f64>::try_into(value)
        {
            let micros = ((self.now_nanos() - sent) / 1000.0).max(0.0) as u32;
            self.latencies.lock().unwrap().push(micros);
        }
    }

    fn drop_equip(&self, equip: u32) {
        self.subs.lock().unwrap().retain(|(e, _), _| *e != equip);
    }
}

/// One per equipment, registered as `db:<equip>` and managed by that
/// equipment's engine alone, so one engine's stop/reset never touches
/// another instance's subscriptions.
struct ScopedConnector {
    equip: u32,
    backend: Arc<Backend>,
}

impl Connector for ScopedConnector {
    fn stop(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async move {
            // Dropping the senders ends this instance's streams only.
            self.backend.drop_equip(self.equip);
            Ok(())
        })
    }

    fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream> {
        let address = address.to_owned();
        Box::pin(async move {
            let point = Point::parse(&address)
                .ok_or_else(|| ConnectorError::subscribe(&address, "unknown point"))?;
            let (tx, rx) = unbounded();
            self.backend
                .subs
                .lock()
                .unwrap()
                .entry((self.equip, point))
                .or_default()
                .push(tx);
            Ok(Box::pin(rx) as ValueStream)
        })
    }

    fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()> {
        self.backend.on_publish(address, &value);
        Box::pin(async { Ok(()) })
    }

    fn request(&self, address: &str, _value: Value) -> ConnectorFuture<'_, Value> {
        let address = address.to_owned();
        Box::pin(async move { Err(ConnectorError::request(&address, "not supported")) })
    }
}

// ---------------------------------------------------------------------
// The VAV template
// ---------------------------------------------------------------------

/// A VAV-sized program: 20 blocks, 5 subscriptions, 5 publish points,
/// two PID loops ticking at 1 s.
fn vav_template() -> logic_mesh::Result<Program> {
    let mut b = ProgramBuilder::new("VAV template");
    let db = || Value::make_str("db");

    let ext_in = |b: &mut ProgramBuilder, addr: &str| -> logic_mesh::Result<_> {
        Ok(b.add_block("ExternalIn")?
            .input("connector", db())?
            .input("address", Value::make_str(addr))?
            .finish())
    };
    let zone_temp = ext_in(&mut b, "zoneTemp")?;
    let zone_sp = ext_in(&mut b, "zoneSp")?;
    let dat = ext_in(&mut b, "dat")?;
    let occ = ext_in(&mut b, "occ")?;
    let probe = ext_in(&mut b, "probe")?;

    let ext_out = |b: &mut ProgramBuilder, addr: &str| -> logic_mesh::Result<_> {
        Ok(b.add_block("ExternalOut")?
            .input("connector", db())?
            .input("address", Value::make_str(addr))?
            .finish())
    };

    // Cooling: smoothed zone temp → PID → min-flow floor → clamp → damper.
    let avg = b.add_block("MovingAverage")?.finish();
    let cool_pid = b.add_block("Pid")?.input("interval", 1000)?.finish();
    let min_flow = b.add_block("Max")?.input("b", 20)?.finish();
    let damper_clamp = b
        .add_block("Clamp")?
        .input("min", 0)?
        .input("max", 100)?
        .finish();
    let damper_cmd = ext_out(&mut b, "damperCmd")?;
    b.link(zone_temp, "out", avg, "in")?;
    b.link(avg, "out", cool_pid, "input")?;
    b.link(zone_sp, "out", cool_pid, "sp")?;
    b.link(cool_pid, "out", min_flow, "a")?;
    b.link(min_flow, "out", damper_clamp, "in")?;
    b.link(damper_clamp, "out", damper_cmd, "in")?;

    // Heating: PID → clamp → gated by occupancy → reheat valve.
    let heat_pid = b.add_block("Pid")?.input("interval", 1000)?.finish();
    let heat_clamp = b
        .add_block("Clamp")?
        .input("min", 0)?
        .input("max", 100)?
        .finish();
    let occ_gate = b.add_block("Mul")?.finish();
    let reheat_cmd = ext_out(&mut b, "reheatCmd")?;
    b.link(zone_temp, "out", heat_pid, "input")?;
    b.link(zone_sp, "out", heat_pid, "sp")?;
    b.link(heat_pid, "out", heat_clamp, "in")?;
    b.link(heat_clamp, "out", occ_gate, "in0")?;
    b.link(occ, "out", occ_gate, "in1")?;
    b.link(occ_gate, "out", reheat_cmd, "in")?;

    // Mode: zone error through a deadband.
    let err = b.add_block("Sub")?.finish();
    let deadband = b
        .add_block("Deadband")?
        .input("high", 1)?
        .input("low", -1)?
        .finish();
    let cooling_mode = ext_out(&mut b, "coolingMode")?;
    b.link(zone_temp, "out", err, "a")?;
    b.link(zone_sp, "out", err, "b")?;
    b.link(err, "out", deadband, "in")?;
    b.link(deadband, "out", cooling_mode, "in")?;

    // Alarm: discharge air too hot.
    let dat_high = b.add_block("GreaterThan")?.input("in2", 30)?.finish();
    let dat_alarm = ext_out(&mut b, "datAlarm")?;
    b.link(dat, "out", dat_high, "in1")?;
    b.link(dat_high, "out", dat_alarm, "in")?;

    // Latency probe.
    let pass = b.add_block("Add")?.input("in1", 0)?.finish();
    let probe_out = ext_out(&mut b, "probeOut")?;
    b.link(probe, "out", pass, "in0")?;
    b.link(pass, "out", probe_out, "in")?;

    Ok(b.build())
}

/// Binds the template to one equipment: the only per-instance change.
fn instantiate(template: &Program, equip: u32) -> Program {
    let mut program = template.clone();
    program.name = Some(format!("VAV-{equip}"));
    let connector = Value::make_str(&format!("db:{equip}"));
    for block in program.blocks.values_mut() {
        if let Some(pin) = block.inputs.get_mut("connector") {
            pin.value = connector.clone();
        }
    }
    program
}

// ---------------------------------------------------------------------
// Feeder: simulated point changes, pushed from a thread of its own
// ---------------------------------------------------------------------

struct Rates {
    /// Seconds between changes of each data point of each equipment.
    data_period: f64,
    /// Seconds between probes of each equipment.
    probe_period: f64,
}

fn run_feeder(backend: Arc<Backend>, engines: u32, rates: Rates, until: Instant) {
    const TICK: Duration = Duration::from_millis(10);
    let tick_s = TICK.as_secs_f64();
    let data_items: Vec<(u32, Point)> = (0..engines)
        .flat_map(|e| Point::DATA.iter().map(move |p| (e, *p)))
        .collect();
    let mut data_budget = 0.0;
    let mut probe_budget = 0.0;
    let mut data_cursor = 0usize;
    let mut probe_cursor = 0u32;
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut next = Instant::now();

    while Instant::now() < until {
        data_budget += data_items.len() as f64 * tick_s / rates.data_period;
        probe_budget += engines as f64 * tick_s / rates.probe_period;

        while data_budget >= 1.0 {
            data_budget -= 1.0;
            let (equip, point) = data_items[data_cursor];
            data_cursor = (data_cursor + 1) % data_items.len();
            let r = rng.next_f64();
            let value = match point {
                Point::ZoneTemp => 20.0 + r * 4.0,
                Point::ZoneSp => 21.5 + r,
                Point::Dat => 12.0 + r * 25.0,
                Point::Occ => (r > 0.5) as u8 as f64,
                Point::Probe => unreachable!(),
            };
            backend.push(equip, point, Value::make_number(value));
        }
        while probe_budget >= 1.0 {
            probe_budget -= 1.0;
            let stamp = backend.now_nanos();
            backend.push(probe_cursor, Point::Probe, Value::make_number(stamp));
            probe_cursor = (probe_cursor + 1) % engines;
        }

        next += TICK;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
}

struct XorShift(u64);
impl XorShift {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ---------------------------------------------------------------------
// Process stats (via `ps`, to stay dependency-free)
// ---------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Sample {
    at: Instant,
    rss_mb: f64,
    cpu_s: f64,
}

fn sample() -> Sample {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=,time=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut fields = text.split_whitespace();
    let rss_kb: f64 = fields.next().and_then(|f| f.parse().ok()).unwrap_or(0.0);
    // `[[hh:]mm:]ss[.ff]`
    let cpu_s = fields
        .next()
        .map(|t| {
            t.split(':').fold(0.0, |acc, part| {
                acc * 60.0 + part.parse::<f64>().unwrap_or(0.0)
            })
        })
        .unwrap_or(0.0);
    Sample {
        at: Instant::now(),
        rss_mb: rss_kb / 1024.0,
        cpu_s,
    }
}

/// CPU used between two samples, as % of one core.
fn cpu_pct(a: Sample, b: Sample) -> f64 {
    100.0 * (b.cpu_s - a.cpu_s) / (b.at - a.at).as_secs_f64()
}

fn percentile(sorted: &[u32], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx] as f64 / 1000.0
}

// ---------------------------------------------------------------------
// Phases
// ---------------------------------------------------------------------

fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn measured_phase(
    label: &str,
    backend: &Arc<Backend>,
    engines: u32,
    rates: Option<Rates>,
    secs: u64,
) {
    backend.latencies.lock().unwrap().clear();
    backend.recording.store(rates.is_some(), Ordering::Relaxed);
    let pushes0 = backend.pushes.load(Ordering::Relaxed);
    let pubs0 = backend.publishes.load(Ordering::Relaxed);
    let s0 = sample();
    let until = Instant::now() + Duration::from_secs(secs);

    match rates {
        Some(rates) => {
            let backend = backend.clone();
            std::thread::spawn(move || run_feeder(backend, engines, rates, until))
                .join()
                .expect("feeder");
        }
        None => std::thread::sleep(Duration::from_secs(secs)),
    }
    // Let in-flight probes land before reading the latencies.
    std::thread::sleep(Duration::from_millis(200));
    backend.recording.store(false, Ordering::Relaxed);

    let s1 = sample();
    let dt = (s1.at - s0.at).as_secs_f64();
    let pushes = backend.pushes.load(Ordering::Relaxed) - pushes0;
    let pubs = backend.publishes.load(Ordering::Relaxed) - pubs0;
    let mut lat = std::mem::take(&mut *backend.latencies.lock().unwrap());
    lat.sort_unstable();

    print!(
        "  {label:<7} cpu {:6.1}%  rss {:7.1} MB  in {:8.0}/s  out {:8.0}/s",
        cpu_pct(s0, s1),
        s1.rss_mb,
        pushes as f64 / dt,
        pubs as f64 / dt,
    );
    if lat.is_empty() {
        println!();
    } else {
        println!(
            "  latency ms p50 {:.2}  p95 {:.2}  p99 {:.2}  max {:.2}  (n={})",
            percentile(&lat, 0.50),
            percentile(&lat, 0.95),
            percentile(&lat, 0.99),
            percentile(&lat, 1.0),
            lat.len()
        );
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let engines: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(500);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let workers: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(cores);
    let (idle_s, load_s, stress_s) = (
        env_secs("BENCH_IDLE", 10),
        env_secs("BENCH_LOAD", 20),
        env_secs("BENCH_STRESS", 15),
    );

    let scan_ms = env_secs("BENCH_SCAN", 200);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()?;

    let template = vav_template()?;
    let backend = Arc::new(Backend::new());
    let base = sample();

    println!(
        "== {engines} engines × {} blocks, {workers} worker threads ({cores} cores), scan {scan_ms} ms",
        template.blocks.len()
    );
    println!("  baseline rss {:7.1} MB", base.rss_mb);

    // --- start-up -----------------------------------------------------
    let started = Instant::now();
    let (clients, handles) = rt.block_on(async {
        let mut clients = Vec::with_capacity(engines as usize);
        let mut handles = Vec::with_capacity(engines as usize);
        for equip in 0..engines {
            let mut engine = MultiThreadedEngine::new();
            engine.set_scan_interval(Duration::from_millis(scan_ms));
            engine.add_connector(
                &format!("db:{equip}"),
                ConnectorHandle::new(ScopedConnector {
                    equip,
                    backend: backend.clone(),
                }),
            )?;
            let client = engine.client();
            handles.push(tokio::spawn(async move {
                engine.run().await;
            }));
            client.load_program(instantiate(&template, equip)).await?;
            clients.push(client);
        }
        anyhow::Ok((clients, handles))
    })?;
    let loaded = started.elapsed();

    let want = engines as usize * SUBS_PER_INSTANCE;
    while backend.live_subscriptions() < want {
        if started.elapsed() > Duration::from_secs(120) {
            anyhow::bail!(
                "only {} of {want} subscriptions after 120 s",
                backend.live_subscriptions()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let subscribed = started.elapsed();
    let up = sample();
    println!(
        "  startup loaded in {:.2} s, all {want} subscriptions live in {:.2} s; \
         rss {:.1} MB (+{:.1} KB/engine)",
        loaded.as_secs_f64(),
        subscribed.as_secs_f64(),
        up.rss_mb,
        (up.rss_mb - base.rss_mb) * 1024.0 / engines as f64,
    );

    // --- steady-state phases ------------------------------------------
    measured_phase(
        "load",
        &backend,
        engines,
        Some(Rates {
            data_period: 5.0,
            probe_period: 1.0,
        }),
        load_s,
    );
    // Idle after load: every loop has values, so the PIDs keep ticking
    // and publishing; only the point feed is quiet.
    measured_phase("idle", &backend, engines, None, idle_s);
    measured_phase(
        "stress",
        &backend,
        engines,
        Some(Rates {
            data_period: 0.25,
            probe_period: 0.25,
        }),
        stress_s,
    );

    // --- shutdown -----------------------------------------------------
    let t = Instant::now();
    rt.block_on(async {
        for client in &clients {
            let _ = client.shutdown().await;
        }
        for handle in handles {
            let _ = handle.await;
        }
    });
    println!(
        "  shutdown {:.2} s, live subscriptions after: {}",
        t.elapsed().as_secs_f64(),
        backend.live_subscriptions()
    );
    Ok(())
}
