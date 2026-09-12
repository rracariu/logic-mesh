//! Connector demo: bridging an external system into Logic Mesh.
//!
//! The library ships only the integration *contract*: the
//! [`Connector`] trait, the process-wide connector registry, and the
//! generic `ExternalIn` / `ExternalOut` / `Request` blocks. Concrete
//! protocol adapters — MQTT, HTTP, WebSocket, or the simulated
//! in-process transport below — live in application code like this
//! example. Registering the adapter under a name is all it takes for
//! the engine's external blocks to use it; the engine itself never
//! learns the protocol.
//!
//! The demo wires all three external data paths through one connector:
//!
//! ```text
//! [sim sensor]  --subscribe-->  ExternalIn --> Add(+0.5) --> ExternalOut --publish--> [sim actuator]
//!                                                            (echoes on out)
//!                                                                 |
//!                                                                 v
//!                              [sim service] <--request/response-- Request
//! ```
//!
//! Run with:
//!
//! ```bash
//! cargo run --example connector_demo
//! ```
//!
//! Everything runs on the `SingleThreadedEngine`, which manages the
//! connector's lifecycle: `run()` awaits the connector's `start`
//! before any block executes, and `Shutdown` awaits its `stop`. A
//! small driver task watches the pin graph (the same `WatchBlockSubReq`
//! primitive the web editor and the `tui_runner` example build on),
//! prints every output change, and shuts the engine down after a few
//! seconds so the demo terminates on its own.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::channel::mpsc as stream_mpsc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use uuid::Uuid;

use logic_mesh::base::Status;
use logic_mesh::base::block::{BlockProps, connect::connect_output};
use logic_mesh::base::connector::{Connector, ConnectorFuture, ConnectorHandle, ValueStream};
use logic_mesh::base::engine::Engine;
use logic_mesh::base::engine::messages::{ChangeSource, EngineMessage, WatchMessage};
use logic_mesh::blocks::external::{ExternalIn, ExternalOut, Request};
use logic_mesh::blocks::math::Add;
use logic_mesh::single_threaded::SingleThreadedEngine;
use logic_mesh::{ConnectorError, Value};

/// Name the connector is registered under. The external blocks resolve
/// it by this name at execution time, so the same block graph works
/// with any adapter registered as `"building-sim"`.
const CONNECTOR_NAME: &str = "building-sim";

/// Address the `ExternalIn` block subscribes to.
const SENSOR_ADDRESS: &str = "sensors/zone-1/temperature";

/// Address the `ExternalOut` block publishes the calibrated value to.
const ACTUATOR_ADDRESS: &str = "actuators/zone-1/heater-setpoint";

/// Address the `Request` block round-trips through.
const SERVICE_ADDRESS: &str = "services/celsius-to-fahrenheit";

/// How long the sampler waits between simulated readings.
const SAMPLE_EVERY: Duration = Duration::from_millis(300);

/// How long the demo runs before shutting the engine down.
const RUN_FOR: Duration = Duration::from_secs(3);

/// The feed side of one live subscription: the address it targets and
/// the sender half of the stream the subscribing block holds. When the
/// block drops its stream — the trait's unsubscribe — the sender
/// observes the closure and the sampler prunes the entry.
type Subscriber = (
    String,
    stream_mpsc::UnboundedSender<Result<Value, ConnectorError>>,
);

/// A simulated building transport: a periodic temperature sensor, an
/// actuator that logs what it is told, and a unit-conversion service.
///
/// This is what a real protocol adapter looks like from the trait's
/// point of view. The connector takes `&self` everywhere — one handle
/// is shared by every block bound to it — so all mutable state sits
/// behind interior mutability. The sampling loop is the connector's
/// own background task: spawned in `start`, wound down in `stop`, as
/// the trait docs prescribe.
struct SimulatedSensorConnector {
    /// Feed sides of every open subscription. Shared with the sampler
    /// task, which pushes a fresh reading to each on every tick.
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
    /// The sampler task plus its shutdown signal, present while the
    /// connector is started. `stop` takes it, signals, and awaits the
    /// task, so a later `start` (the restart contract) begins clean.
    sampler: Mutex<Option<(watch::Sender<()>, JoinHandle<()>)>>,
}

impl SimulatedSensorConnector {
    fn new() -> Self {
        Self {
            subscribers: Arc::default(),
            sampler: Mutex::new(None),
        }
    }
}

/// Deterministic sawtooth between 20.0 and 21.75 so every tick yields
/// a fresh value (unchanged values would not propagate: pin flow is
/// change-of-value) and the run is reproducible.
fn simulated_reading(step: u64) -> Value {
    Value::make_number(20.0 + (step % 8) as f64 * 0.25)
}

impl Connector for SimulatedSensorConnector {
    fn start(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async move {
            // The engine awaits `start` before driving any block, so
            // the sampler is live before the first subscribe arrives.
            let (stop_tx, mut stop_rx) = watch::channel(());
            let subscribers = self.subscribers.clone();
            let task = tokio::spawn(async move {
                let mut ticker = tokio::time::interval(SAMPLE_EVERY);
                let mut step = 0u64;
                loop {
                    tokio::select! {
                        _ = stop_rx.changed() => break,
                        _ = ticker.tick() => {
                            step += 1;
                            // Route the reading to the subscribers of
                            // the sensor address only — a subscription
                            // to any other address stays open but
                            // silent, as a real protocol adapter would
                            // route by topic. Streams the blocks have
                            // dropped are pruned (dropping the stream
                            // is the unsubscribe).
                            subscribers
                                .lock()
                                .expect("subscriber list poisoned")
                                .retain(|(address, tx)| {
                                    if tx.is_closed() {
                                        return false;
                                    }
                                    if address != SENSOR_ADDRESS {
                                        return true;
                                    }
                                    tx.unbounded_send(Ok(simulated_reading(step))).is_ok()
                                });
                        }
                    }
                }
            });
            *self.sampler.lock().expect("sampler slot poisoned") = Some((stop_tx, task));
            println!("[sim ] transport started");
            Ok(())
        })
    }

    fn stop(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async move {
            let sampler = self.sampler.lock().expect("sampler slot poisoned").take();
            if let Some((stop_tx, _)) = &sampler {
                let _ = stop_tx.send(());
            }
            // End every outstanding stream (the holders observe the
            // `None` and re-subscribe after a restart), per the
            // trait's stop contract: dropping the senders closes the
            // streams. Cleared BEFORE awaiting the sampler — this
            // future may itself be dropped mid-await (the engine
            // bounds `stop` with a timeout), and the streams must end
            // even then.
            self.subscribers
                .lock()
                .expect("subscriber list poisoned")
                .clear();
            if let Some((_, task)) = sampler {
                let _ = task.await;
            }
            println!("[sim ] transport stopped");
            Ok(())
        })
    }

    fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream> {
        let address = address.to_string();
        Box::pin(async move {
            let (tx, rx) = stream_mpsc::unbounded();
            // Seed the sensor's subscribers with one reading
            // immediately, so they see data without waiting for the
            // next sampler tick. The value sits below the sawtooth's
            // 20.00..=21.75 range so the first sampled reading always
            // differs from it — pin flow is change-of-value, and a
            // duplicate would be swallowed.
            if address == SENSOR_ADDRESS {
                let _ = tx.unbounded_send(Ok(Value::make_number(19.75)));
            }
            self.subscribers
                .lock()
                .expect("subscriber list poisoned")
                .push((address, tx));
            Ok(Box::pin(rx) as ValueStream)
        })
    }

    fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()> {
        let address = address.to_string();
        Box::pin(async move {
            // A real adapter would hand the value to its protocol here.
            println!("[sim ] actuator '{address}' set to {}", fmt_value(&value));
            Ok(())
        })
    }

    fn request(&self, address: &str, value: Value) -> ConnectorFuture<'_, Value> {
        let address = address.to_string();
        Box::pin(async move {
            // The simulated service converts Celsius to Fahrenheit.
            // Failures are reported through `ConnectorError`, which the
            // `Request` block surfaces as a block fault.
            let celsius: f64 = (&value).try_into().map_err(|_| ConnectorError::Request {
                address: address.clone(),
                detail: format!("expected a number, got {value:?}"),
            })?;
            let fahrenheit = celsius * 9.0 / 5.0 + 32.0;
            println!("[sim ] service '{address}' answered {celsius:.2} C -> {fahrenheit:.2} F");
            Ok(Value::make_number(fahrenheit))
        })
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // ----- 1. Register the connector with the engine -----
    // `add_connector` puts the handle in the process-wide registry and
    // under the engine's lifecycle management: `run()` awaits `start`
    // before any block executes, shutdown awaits `stop`.
    let mut engine = SingleThreadedEngine::new();
    engine
        .add_connector(
            CONNECTOR_NAME,
            ConnectorHandle::new(SimulatedSensorConnector::new()),
        )
        .expect("connector name is free");

    // ----- 2. Build the block graph -----
    // The external blocks bind by *pin values*, not by handle: the
    // `connector` and `address` pins are ordinary string inputs, so a
    // program could rewire them at runtime like any other value.
    let mut sensor = ExternalIn::new();
    sensor.connector.val = Some(Value::make_str(CONNECTOR_NAME));
    sensor.address.val = Some(Value::make_str(SENSOR_ADDRESS));

    // A tiny bit of local processing between input and output: add a
    // fixed calibration offset to each reading.
    let mut calibrate = Add::new();
    calibrate.inputs_mut()[1].set_value(Value::make_number(0.5), Status::Ok);

    let mut publish = ExternalOut::new();
    publish.connector.val = Some(Value::make_str(CONNECTOR_NAME));
    publish.address.val = Some(Value::make_str(ACTUATOR_ADDRESS));

    let mut convert = Request::new();
    convert.connector.val = Some(Value::make_str(CONNECTOR_NAME));
    convert.address.val = Some(Value::make_str(SERVICE_ADDRESS));
    convert.timeout.val = Some(Value::make_number(1000.0)); // milliseconds

    // sensor -> calibrate -> publish -> convert. `ExternalOut` echoes
    // the published value on `out`, so chaining the `Request` block off
    // it round-trips exactly the values that reached the actuator.
    connect_output(&mut sensor.out, calibrate.inputs_mut()[0]).expect("wire sensor -> calibrate");
    connect_output(&mut calibrate.out, &mut publish.input).expect("wire calibrate -> publish");
    connect_output(&mut publish.out, &mut convert.input).expect("wire publish echo -> convert");

    // Remember ids before `schedule` moves each block into its actor
    // task, so the watcher below can print human-readable names.
    let names: BTreeMap<Uuid, &str> = [
        (*sensor.id(), "sensor"),
        (*calibrate.id(), "calibrate"),
        (*publish.id(), "publish"),
        (*convert.id(), "convert"),
    ]
    .into();

    engine.schedule(sensor).expect("schedule sensor");
    engine.schedule(calibrate).expect("schedule calibrate");
    engine.schedule(publish).expect("schedule publish");
    engine.schedule(convert).expect("schedule convert");

    // ----- 3. Watch the flow and bound the runtime -----
    // `run()` only returns on `EngineMessage::Shutdown`, so a driver
    // task subscribes to change-of-value notifications, prints them,
    // and pulls the plug after `RUN_FOR`.
    let (reply_tx, mut reply_rx) = mpsc::channel(32);
    let channel_id = Uuid::new_v4();
    let engine_sender = engine.create_message_channel(channel_id, reply_tx);

    tokio::spawn(async move {
        let (watch_tx, mut watch_rx) = mpsc::unbounded_channel::<WatchMessage>();
        engine_sender
            .send(EngineMessage::WatchBlockSubReq(channel_id, watch_tx))
            .await
            .expect("engine is receiving");
        let _ = reply_rx.recv().await; // drain the WatchBlockSubRes

        let deadline = tokio::time::Instant::now() + RUN_FOR;
        loop {
            tokio::select! {
                Some(msg) = watch_rx.recv() => print_watch(&names, msg),
                _ = tokio::time::sleep_until(deadline) => break,
            }
        }

        println!("[demo] time is up, shutting the engine down");
        let _ = engine_sender.send(EngineMessage::Shutdown).await;
    });

    engine.run().await;
    println!("[demo] engine stopped cleanly");
}

/// Prints the output-pin changes (and any fault) a watch notification
/// carries. Input changes are skipped — for this linear graph they
/// mirror the upstream outputs.
fn print_watch(names: &BTreeMap<Uuid, &str>, msg: WatchMessage) {
    let block = names.get(&msg.block_id).copied().unwrap_or("?");
    if let Some(reason) = msg.state.fault_reason() {
        println!("[flow] {block:<9} FAULT: {reason}");
    }
    for change in msg.changes.into_values() {
        if let ChangeSource::Output(pin, value) = change {
            println!("[flow] {block:<9} {pin} = {}", fmt_value(&value));
        }
    }
}

/// Renders a pin value compactly: numbers with two decimals, anything
/// else via `Debug`.
fn fmt_value(value: &Value) -> String {
    match TryInto::<f64>::try_into(value) {
        Ok(num) => format!("{num:.2}"),
        Err(_) => format!("{value:?}"),
    }
}
