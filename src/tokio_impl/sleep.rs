// Copyright (c) 2022-2024, Radu Racariu.

// The engine's timers and clocks on wasm32 come from the JavaScript
// host. They are part of the engine, not of the `js` bindings, so a
// wasm32 build without that feature has them too.
#[cfg(target_arch = "wasm32")]
mod wasm32;
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm32::{current_time_millis, monotonic_millis, sleep_millis, yield_to_host};

/// Milliseconds on a monotonic clock (non-WASM version): only the
/// difference between two readings means anything, and wall-clock
/// changes do not move it, so it is what deadlines are measured on.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn monotonic_millis() -> u64 {
    use std::sync::OnceLock;
    use std::time::Instant;

    static ORIGIN: OnceLock<Instant> = OnceLock::new();
    ORIGIN.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// Sleeps for the given number of milliseconds (non-WASM version).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn sleep_millis(millis: u64) {
    use tokio::time::{Duration, sleep};

    sleep(Duration::from_millis(millis)).await;
}

/// Returns the current wall-clock time in milliseconds since the Unix epoch.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn current_time_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        Err(_) => 0,
    }
}
