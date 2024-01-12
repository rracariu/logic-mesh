// Copyright (c) 2022-2024, Radu Racariu.

#[cfg(target_arch = "wasm32")]
pub(super) use crate::wasm::sleep::sleep_millis;

/// Sleep for a given number of milliseconds
/// This function is used to wait for a given amount of time
/// This is the non-wasm version
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn sleep_millis(millis: u64) {
    use tokio::time::{sleep, Duration};

    sleep(Duration::from_millis(millis)).await;
}
