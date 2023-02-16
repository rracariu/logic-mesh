// Copyright (c) 2022-2023, IntriSemantics Corp.

/// Default value for sleep intervals
pub(super) const DEFAULT_SLEEP_DUR: u64 = 200;

#[cfg(target_arch = "wasm32")]
pub(super) async fn sleep_millis(millis: u64) {
    use wasm_bindgen_futures::JsFuture;

    let millis: i32 = millis.try_into().expect("Conversion to millis");

    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .expect("Window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Future");
    });

    let _ = JsFuture::from(promise).await;
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn sleep_millis(millis: u64) {
    use tokio::time::{sleep, Duration};

    sleep(Duration::from_millis(millis)).await;
}
