// Copyright (c) 2022-2023, IntriSemantics Corp.

use js_sys::Array;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::blocks::registry::BLOCKS;

use super::types::{Desc, EngineWrap};

#[wasm_bindgen(js_name = "listBlocks")]
/// Lists all available blocks
pub fn list_blocks() -> Array {
    let arr = Array::new();

    BLOCKS.iter().for_each(|(_, block)| {
        let desc = Desc {
            name: block.name.clone(),
            lib: block.library.clone(),
        };

        if let Ok(desc) = serde_wasm_bindgen::to_value(&desc) {
            arr.push(&desc);
        }
    });

    arr
}

#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine() -> EngineWrap {
    EngineWrap::new()
}
