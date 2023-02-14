// Copyright (c) 2022-2023, IntriSemantics Corp.

use js_sys::Array;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::blocks::registry::BLOCKS;

use super::types::{BlockFieldProp, BlockProperties, BlocksEngine};

#[wasm_bindgen(js_name = "listBlocks")]
/// Lists all available blocks
pub fn list_blocks() -> Array {
    let arr = Array::new();

    BLOCKS.iter().for_each(|(_, desc)| {
        let desc = BlockProperties {
            name: desc.name.clone(),
            lib: desc.library.clone(),
            doc: desc.doc.clone(),
            inputs: desc
                .inputs
                .iter()
                .map(|input| BlockFieldProp {
                    name: input.name.clone(),
                    kind: input.kind.to_string(),
                })
                .collect(),
            output: BlockFieldProp {
                name: desc.output.name.clone(),
                kind: desc.output.kind.to_string(),
            },
        };

        if let Ok(desc) = serde_wasm_bindgen::to_value(&desc) {
            arr.push(&desc);
        }
    });

    arr
}

#[wasm_bindgen(js_name = "initEngine")]
pub fn init_engine() -> BlocksEngine {
    BlocksEngine::new()
}
