// Copyright (c) 2022-2023, IntriSemantics Corp.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Engine;

#[derive(Default, Serialize, Deserialize)]
pub struct BlockFieldProp {
    pub name: String,
    pub kind: String,
}

#[derive(Default, Serialize, Deserialize)]
/// Block properties
pub struct BlockProperties {
    pub name: String,
    pub lib: String,
    pub inputs: Vec<BlockFieldProp>,
    pub output: BlockFieldProp,
}

#[wasm_bindgen]
pub struct BlocksEngine {
    engine: Engine,
}

#[wasm_bindgen]
impl BlocksEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    #[wasm_bindgen]
    pub async fn run(&mut self) {
        self.engine.run().await;
    }
}
