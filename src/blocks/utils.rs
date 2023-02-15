// Copyright (c) 2022-2023, IntriSemantics Corp.

use wasm_bindgen_futures::JsFuture;

/// Default value for sleep intervals
pub(super) const DEFAULT_SLEEP_DUR: u64 = 200;

pub(super) async fn sleep_millis(millis: u64) {
    let millis: i32 = millis.try_into().expect("Conversion to millis");

    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .expect("Window")
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .expect("Future");
    });

    let _ = JsFuture::from(promise).await;
}
