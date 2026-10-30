// Copyright (c) 2022-2024, Radu Racariu.

use std::cell::Cell;

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

/// How long, in milliseconds, the engine's tasks may keep the host's
/// event loop from running anything else before [`yield_to_host`]
/// hands it a turn.
const HOST_SLICE_MILLIS: u64 = 10;

thread_local! {
    /// When the pending host-turn probe was scheduled, while there is
    /// one: a `setTimeout(0)` task that clears this once the event loop
    /// gets to run it. Set for longer than [`HOST_SLICE_MILLIS`], the
    /// engine has kept the event loop from running anything else.
    static PROBE_SINCE: Cell<Option<u64>> = const { Cell::new(None) };
}

/// Hands the host's event loop a turn if the engine's tasks have kept
/// it from running anything else for [`HOST_SLICE_MILLIS`]: waits for a
/// `setTimeout(0)` then, and returns at once otherwise.
///
/// The engine's tasks are woken through microtasks, which the host runs
/// before any timer, I/O callback or rendering. A block that never
/// waits on a timer, such as one fed by its own output, would wake on
/// microtasks forever and freeze the page or process; a wait on a
/// timer, which is a macrotask, lets the host run what has queued up.
/// Most calls return at once, so blocks that react quickly lose
/// nothing to it.
pub(crate) async fn yield_to_host() {
    let now = monotonic_millis();
    match PROBE_SINCE.get() {
        None => {
            PROBE_SINCE.set(Some(now));
            wasm_bindgen_futures::spawn_local(async {
                sleep_millis(0).await;
                PROBE_SINCE.set(None);
            });
        }
        Some(since) if now.saturating_sub(since) >= HOST_SLICE_MILLIS => sleep_millis(0).await,
        Some(_) => {}
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
