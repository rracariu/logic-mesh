//! Minimal Logic Mesh example.
//!
//! Wires two `SineWave` blocks into an `Add`, drives the chain by hand
//! for a few cycles, and prints the adder's output value.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example minimal
//! ```
//!
//! This deliberately bypasses the engine and the actor scheduler.
//! In a real deployment you would schedule blocks on a
//! `SingleThreadedEngine` (or `MultiThreadedEngine` with
//! `--features multi-threaded`) and let the per-block actor tasks
//! run concurrently. The `tui_runner` example shows that path; this
//! one keeps everything inline so the focus stays on the `Block` /
//! `BlockProps` / `connect_output` API surface.
//!
//! Pin connections in this crate are typed watch channels. `Sine -> Add`
//! pushes new values into the adder's input watch channel; `Add::execute`
//! drains them on its next cycle.

use logic_mesh::base::block::{Block, BlockProps, connect::connect_output};
use logic_mesh::blocks::{math::Add, misc::SineWave};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Two sine sources with different amplitudes + periods. Period is
    // in milliseconds (`freq` is "ms per sample" in this block).
    let mut sine_fast = SineWave::new();
    sine_fast.amplitude.val = Some(3.into());
    sine_fast.freq.val = Some(50.into());

    let mut sine_slow = SineWave::new();
    sine_slow.amplitude.val = Some(7.into());
    sine_slow.freq.val = Some(50.into());

    let mut adder = Add::new();

    connect_output(&mut sine_fast.out, adder.inputs_mut()[0])
        .expect("wire sine_fast -> adder.in0");
    connect_output(&mut sine_slow.out, adder.inputs_mut()[1])
        .expect("wire sine_slow -> adder.in1");

    println!("cycle | sine_fast |  sine_slow |   sum");
    println!("------+-----------+------------+----------");

    for cycle in 0..10 {
        // Drive the producers first, then the consumer. In an engine-
        // hosted run the actor scheduler interleaves these; here we
        // step through them by hand.
        sine_fast.execute().await;
        sine_slow.execute().await;
        adder.execute().await;

        let a = pin_to_f64(&sine_fast.out.value);
        let b = pin_to_f64(&sine_slow.out.value);
        let sum = pin_to_f64(&adder.out.value);
        println!("{cycle:5} | {a:9.3} | {b:10.3} | {sum:8.3}");
    }
}

/// Extract a numeric pin value or 0 if the block has not emitted yet.
fn pin_to_f64(v: &libhaystack::val::Value) -> f64 {
    v.try_into().unwrap_or(0.0)
}
