// Copyright (c) 2022-2023, Radu Racariu.

//! WebAssembly bindings: the JavaScript API of the `logic-mesh` npm
//! package.
//!
//! Built on `wasm32` with the default `js` feature. A Rust application
//! that builds this crate for `wasm32` and brings its own bindings
//! turns the feature off (`default-features = false`): it then gets
//! neither these exports nor the module start hook, which installs a
//! panic hook and a console logger.

pub mod engine;
pub mod engine_command;
pub(crate) mod error;
pub mod js_block;
pub mod js_connector;
pub mod types;

use std::str::FromStr;
use std::sync::Once;

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

/// Creates and returns a new [`BlocksEngine`].
#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine(sleep_duration: Option<u32>) -> BlocksEngine {
    let engine = BlocksEngine::new(sleep_duration.map(u64::from));
    debug!("Blocks engine initialized.");
    engine
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
pub fn set_log_level(level: &str) -> Result<(), JsValue> {
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
