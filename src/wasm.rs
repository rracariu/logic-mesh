// Copyright (c) 2022-2023, Radu Racariu.

//! WebAssembly bindings.

pub mod engine;
pub mod engine_command;
pub mod js_block;
pub(crate) mod sleep;
pub mod types;

use log::info;
use std::panic;
use wasm_bindgen::prelude::*;
use wasm_bindgen_console_logger::DEFAULT_LOGGER;

use self::engine::BlocksEngine;

/// Creates and returns a new [`BlocksEngine`].
#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine(sleep_duration: Option<u32>) -> BlocksEngine {
    let engine = BlocksEngine::new(sleep_duration.map(u64::from));
    info!("Blocks engine initialized.");
    engine
}

/// WASM module entry point — installs the panic hook and logger.
#[wasm_bindgen(start)]
pub fn start() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    log::set_logger(&DEFAULT_LOGGER).expect("Unable to set default logger.");
    log::set_max_level(log::LevelFilter::Trace);

    info!("Blocks module loaded.");
}
