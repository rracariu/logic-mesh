// Copyright (c) 2022-2023, IntriSemantics Corp.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Engine;

#[derive(Default, Serialize, Deserialize)]
/// Block description
pub struct Desc {
    pub name: String,
    pub lib: String,
}

#[wasm_bindgen]
pub struct EngineWrap {
    engine: Engine,
}

#[wasm_bindgen]
impl EngineWrap {
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
