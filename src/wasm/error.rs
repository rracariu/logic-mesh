// Copyright (c) 2022-2026, Radu Racariu.

//! The errors the JavaScript bindings throw, and the reading of errors
//! JavaScript throws.

use wasm_bindgen::{JsCast, JsValue};

use crate::base::error::{ErrorCode, RequestError};

/// The `name` of every error [`js_error`] builds.
pub(crate) const JS_ERROR_NAME: &str = "LogicMeshError";

/// `err` as a JavaScript `Error` named [`JS_ERROR_NAME`], whose
/// `message` is the request error's message and whose `code` property
/// is its [`ErrorCode`] name — what an engine command's promise rejects
/// with, and what the synchronous bindings throw.
pub(crate) fn js_error(err: RequestError) -> JsValue {
    let error = js_sys::Error::new(&err.message);
    error.set_name(JS_ERROR_NAME);
    // Setting a property on a fresh plain `Error` cannot fail; if it
    // ever did, the error would still carry its message.
    let _ = js_sys::Reflect::set(&error, &"code".into(), &err.code.as_str().into());
    error.into()
}

/// A request error with `code` and `message`, as a JavaScript error.
pub(crate) fn js_error_from(code: ErrorCode, message: impl Into<String>) -> JsValue {
    js_error(RequestError::new(code, message))
}

/// Renders a JS exception or rejection reason as a detail message: an
/// `Error` via its own `toString()` (preserving the error name, e.g.
/// `"TypeError: ..."`), a plain string as-is, anything else via its
/// debug representation.
pub(crate) fn js_detail(err: &JsValue) -> String {
    if let Some(error) = err.dyn_ref::<js_sys::Error>() {
        return String::from(js_sys::Error::to_string(error));
    }

    err.as_string().unwrap_or_else(|| format!("{err:?}"))
}

/// What kind of JavaScript value `value` is, for a message about a value
/// of the wrong kind: its `typeof`, except that `null` is `"null"`.
pub(crate) fn js_type_name(value: &JsValue) -> String {
    if value.is_null() {
        return "null".to_string();
    }
    value.js_typeof().as_string().unwrap_or_default()
}
