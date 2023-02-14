// Copyright (c) 2022-2023, IntriSemantics Corp.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Engine;

/// Block field properties, inputs or output
#[derive(Default, Serialize, Deserialize)]
pub struct BlockFieldProp {
    pub name: String,
    pub kind: String,
}

/// Block properties
#[derive(Default, Serialize, Deserialize)]
pub struct BlockProperties {
    pub name: String,
    pub lib: String,
    pub doc: String,
    pub inputs: Vec<BlockFieldProp>,
    pub output: BlockFieldProp,
}

/// Controls the execution or the blocks.
/// Loads programs and enables inspection and debugging
/// of the blocks and their inputs and outputs.
#[wasm_bindgen]
pub struct BlocksEngine {
    engine: Engine,
}

#[wasm_bindgen]
impl BlocksEngine {
    /// Create a new instance of an engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Runs the engine asynchronously
    #[wasm_bindgen]
    pub async fn run(&mut self) {
        self.engine.run().await;
    }
}
