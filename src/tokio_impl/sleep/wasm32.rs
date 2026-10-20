// Copyright (c) 2022-2024, Radu Racariu.

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// Binds to the global `setTimeout` function.
    #[wasm_bindgen]
    fn setTimeout(handler: &::js_sys::Function, timeout: i32);

    /// Binds to the global `performance.now()` — present in browsers,
    /// workers and Node alike.
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn performance_now() -> f64;
}

/// The longest delay `setTimeout` waits for. Hosts fire a longer one at
/// once, and a delay cast to `i32` would wrap to a short or negative one.
const MAX_TIMEOUT_MILLIS: u64 = i32::MAX as u64;

/// Sleeps for the given number of milliseconds.
///
/// Uses `setTimeout` so it integrates with the browser's or Node's event loop.
/// A sleep longer than `setTimeout` takes is a run of the longest ones.
pub(crate) async fn sleep_millis(millis: u64) {
    let mut remaining = millis;
    loop {
        let timeout = remaining.min(MAX_TIMEOUT_MILLIS);
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            // At most `i32::MAX`, so the cast keeps the value.
            setTimeout(&resolve, timeout as i32);
        });
        let _ = JsFuture::from(promise).await;

        remaining -= timeout;
        if remaining == 0 {
            break;
        }
    }
}

/// Returns the current wall-clock time in milliseconds since the Unix epoch.
///
/// Uses `Date.now()` rather than `performance.now()` — the latter is a
/// monotonic clock relative to page-load and would put time-of-day blocks
/// (Sun, Schedule, Calendar, Now) somewhere in January 1970.
pub(crate) fn current_time_millis() -> u64 {
    js_sys::Date::now() as u64
}

/// Milliseconds on a monotonic clock: `performance.now()`, which only
/// counts from the time origin of the page or process, but which
/// wall-clock changes do not move — what deadlines are measured on.
pub(crate) fn monotonic_millis() -> u64 {
    performance_now() as u64
}
