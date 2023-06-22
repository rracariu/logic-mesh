// Copyright (c) 2022-2023, IntriSemantics Corp.

pub mod engine;
pub mod engine_command;
pub mod types;

use log::info;
use std::panic;
use wasm_bindgen::prelude::*;
use wasm_bindgen_console_logger::DEFAULT_LOGGER;

use self::engine::BlocksEngine;

#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine() -> BlocksEngine {
    let engine = BlocksEngine::new();
    info!("Blocks engine initialized.");
    engine
}

#[wasm_bindgen(start)]
pub fn start() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    log::set_logger(&DEFAULT_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    info!("Blocks module loaded.");
}
