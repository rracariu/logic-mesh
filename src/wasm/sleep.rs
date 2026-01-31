// Copyright (c) 2022-2024, Radu Racariu.

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// Bind this to the global `setTimeout` function
    #[wasm_bindgen]
    fn setTimeout(handler: &::js_sys::Function, timeout: i32);
}

#[wasm_bindgen]
extern "C" {
    # [wasm_bindgen (js_name = Performance, typescript_type = "Performance")]
    type Performance;

    /// Bind this to the global `performance.now` function
    # [wasm_bindgen (method, js_class = "Performance", js_name = now)]
    fn now(this: &Performance) -> f64;

    #[wasm_bindgen(thread_local_v2, js_name = performance)]
    static PERFORMANCE: Performance;
}

/// Sleep for a given number of milliseconds.
/// Uses `setTimeout` function so it integrates with the browser's or node's event loop.
pub(crate) async fn sleep_millis(millis: u64) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        setTimeout(&resolve, millis as i32);
    });

    let _ = JsFuture::from(promise).await;
}

/// Get the current time in milliseconds.
/// Uses `Performance.now` function so it integrates with the browser's or node's event loop.
pub(crate) fn current_time_millis() -> u64 {
    PERFORMANCE.with(|p| p.now()) as u64
}
