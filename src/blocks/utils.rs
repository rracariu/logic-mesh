// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::{
    units::units_generated::MILLISECOND,
    val::{Number, Value},
};

use super::InputImpl;

/// Default value for sleep intervals
pub(super) const DEFAULT_SLEEP_DUR: u64 = 200;

/// Sleep for a given number of milliseconds
/// This function is used to wait for a given amount of time
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

/// Sleep for a given number of milliseconds
/// This function is used to wait for a given amount of time
/// This is the non-wasm version
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn sleep_millis(millis: u64) {
    use tokio::time::{sleep, Duration};

    sleep(Duration::from_millis(millis)).await;
}

pub(super) fn input_as_float_or_default(input: &InputImpl) -> f64 {
    input_as_number(input).map(|v| v.value).unwrap_or(0.0)
}

pub(super) fn input_as_number(input: &InputImpl) -> Option<Number> {
    if let Some(Value::Number(val)) = input.val {
        Some(val)
    } else {
        None
    }
}

pub(super) fn to_millis(dur: &Option<Value>) -> Result<u64, ()> {
    if let Some(Value::Number(dur)) = dur {
        if let Some(unit) = dur.unit {
            match unit.convert_to(dur.value, &MILLISECOND) {
                Ok(millis) => Ok(millis as u64),
                Err(_) => Err(()),
            }
        } else {
            Ok(dur.value as u64)
        }
    } else {
        Err(())
    }
}
