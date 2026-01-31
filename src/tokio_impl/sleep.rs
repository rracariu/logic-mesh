// Copyright (c) 2022-2024, Radu Racariu.

#[cfg(target_arch = "wasm32")]
pub(crate) use crate::wasm::sleep::current_time_millis;
#[cfg(target_arch = "wasm32")]
pub(super) use crate::wasm::sleep::sleep_millis;

/// Sleep for a given number of milliseconds
/// This function is used to wait for a given amount of time
/// This is the non-wasm version
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn sleep_millis(millis: u64) {
    use tokio::time::{Duration, sleep};

    sleep(Duration::from_millis(millis)).await;
}

/// Get the current time in milliseconds
/// This function is used to get the current time
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn current_time_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        Err(_) => 0,
    }
}
