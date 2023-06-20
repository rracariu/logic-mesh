// Copyright (c) 2022-2023, IntriSemantics Corp.

pub mod api;
pub mod engine;
pub mod engine_command;
pub mod types;

// Copyright (c) 2022-2023, IntriSemantics Corp.

use wasm_bindgen::prelude::wasm_bindgen;

use self::engine::BlocksEngine;

#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine() -> BlocksEngine {
    BlocksEngine::new()
}
