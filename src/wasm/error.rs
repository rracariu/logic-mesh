// Copyright (c) 2022-2026, Radu Racariu.

//! The errors the JavaScript bindings throw.

use wasm_bindgen::JsValue;

use crate::base::error::{ErrorCode, RequestError};

/// The `name` of every error [`js_error`] builds.
pub(crate) const JS_ERROR_NAME: &str = "LogicMeshError";

/// `err` as a JavaScript `Error` named [`JS_ERROR_NAME`], whose
/// `message` is the request error's message and whose `code` property
/// is its [`ErrorCode`] name — what an engine command's promise rejects
/// with.
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
