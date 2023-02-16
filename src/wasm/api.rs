// Copyright (c) 2022-2023, IntriSemantics Corp.

use wasm_bindgen::prelude::wasm_bindgen;

use super::types::BlocksEngine;

#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine() -> BlocksEngine {
    BlocksEngine::new()
}
