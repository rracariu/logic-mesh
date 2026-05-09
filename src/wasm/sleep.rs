// Copyright (c) 2022-2024, Radu Racariu.

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// Bind this to the global `setTimeout` function
    #[wasm_bindgen]
    fn setTimeout(handler: &::js_sys::Function, timeout: i32);
}

/// Sleep for a given number of milliseconds.
/// Uses `setTimeout` function so it integrates with the browser's or node's event loop.
pub(crate) async fn sleep_millis(millis: u64) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        setTimeout(&resolve, millis as i32);
    });

    let _ = JsFuture::from(promise).await;
}

/// Get the current wall-clock time in milliseconds since the Unix epoch.
///
/// Uses `Date.now()` rather than `performance.now()` — the latter is a
/// monotonic clock relative to page-load and would put time-of-day blocks
/// (Sun, Schedule, Calendar, Now) somewhere in January 1970.
pub(crate) fn current_time_millis() -> u64 {
    js_sys::Date::now() as u64
}
