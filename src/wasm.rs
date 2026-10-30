// Copyright (c) 2022-2023, Radu Racariu.

//! WebAssembly bindings: the JavaScript API of the `logic-mesh` npm
//! package.
//!
//! Built on `wasm32` with the default `js` feature. A Rust application
//! that builds this crate for `wasm32` and brings its own bindings
//! turns the feature off (`default-features = false`): it then gets
//! neither these exports nor the module start hook, which installs a
//! panic hook and a console logger.
//!
//! The doc comments of the exported items are the TypeScript
//! declarations' docs too, so they name items as JavaScript sees them.

pub mod engine;
pub mod engine_command;
pub(crate) mod error;
pub(crate) mod js_block;
pub mod js_connector;
pub(crate) mod types;

use std::str::FromStr;
use std::sync::Once;
use std::time::Duration;

use log::{LevelFilter, debug};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_console_logger::DEFAULT_LOGGER;

use self::engine::BlocksEngine;
use self::error::js_error_from;
use crate::base::error::ErrorCode;

/// The level the console logger starts at: warnings and errors only.
/// JavaScript raises it with `setLogLevel`.
const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Warn;

/// The largest integer a JavaScript number holds exactly.
pub(crate) const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Creates a new `BlocksEngine`, as its constructor does.
///
/// `scan_interval` is the engine's scan interval in milliseconds: how
/// long a block that runs on every cycle waits between runs. Left out,
/// the engine's default applies. A fraction of a millisecond is
/// dropped.
///
/// Throws a `LogicMeshError` with the `InvalidArgument` code if
/// `scan_interval` is negative, not finite, or above
/// `Number.MAX_SAFE_INTEGER`.
#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine(scan_interval: Option<f64>) -> Result<BlocksEngine, JsValue> {
    let engine = BlocksEngine::new(scan_interval)?;
    debug!("Blocks engine initialized.");
    Ok(engine)
}

/// Sets the most verbose level the module logs to the console at:
/// `'off'`, `'error'`, `'warn'` (the default), `'info'`, `'debug'` or
/// `'trace'`.
///
/// The level is process-wide, as the `log` crate's is: it also limits
/// a logger a Rust application installed instead of the module's own.
///
/// Throws a `LogicMeshError` with the `InvalidArgument` code for any
/// other level.
#[wasm_bindgen(js_name = "setLogLevel")]
pub fn set_log_level(
    #[wasm_bindgen(unchecked_param_type = "LogLevel")] level: &str,
) -> Result<(), JsValue> {
    let level = LevelFilter::from_str(level).map_err(|_| {
        js_error_from(
            ErrorCode::InvalidArgument,
            format!("Invalid log level '{level}': expected off, error, warn, info, debug or trace"),
        )
    })?;
    log::set_max_level(level);
    Ok(())
}

/// The module's start hook, which runs when the module loads: routes
/// panics to `console.error` and installs a console logger at the
/// `'warn'` level.
///
/// Safe to call again, which does nothing. If a logger is already
/// installed, it is kept, and so is its level.
#[wasm_bindgen(start)]
pub fn start() {
    static START: Once = Once::new();
    START.call_once(|| {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        // Another logger — a Rust application's own — is kept: the
        // module must not take logging over, or panic over it.
        if log::set_logger(&DEFAULT_LOGGER).is_ok() {
            log::set_max_level(DEFAULT_LOG_LEVEL);
        }
        debug!("Blocks module loaded.");
    });
}

/// Reads a scan interval in milliseconds from JavaScript.
pub(crate) fn scan_interval(millis: f64) -> Result<Duration, JsValue> {
    if !(millis.is_finite() && (0.0..=MAX_SAFE_INTEGER).contains(&millis)) {
        return Err(js_error_from(
            ErrorCode::InvalidArgument,
            format!(
                "Invalid scan interval {millis}: expected a number of milliseconds from 0 to Number.MAX_SAFE_INTEGER"
            ),
        ));
    }
    Ok(Duration::from_millis(millis as u64))
}
