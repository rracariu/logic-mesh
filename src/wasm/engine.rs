// Copyright (c) 2022-2023, Radu Racariu.

use crate::blocks::registry::{list_registered_blocks, register_block_desc};
use crate::single_threaded::SingleThreadedEngine;
use crate::wasm::engine_command::EngineCommand;
use crate::wasm::js_block::JS_FNS;
use crate::wasm::types::{JsBlockDesc, JsBlockPin};
use js_sys::Array;

use tokio::sync::mpsc;
use uuid::Uuid;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::base::engine::Engine;

/// Controls the execution or the blocks.
/// Loads programs and enables inspection and debugging
/// of the blocks and their inputs and outputs.
#[wasm_bindgen]
pub struct BlocksEngine {
    engine: SingleThreadedEngine,
}

#[wasm_bindgen]
impl BlocksEngine {
    /// Create a new instance of an engine
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            engine: SingleThreadedEngine::default(),
        }
    }

    /// Lists all available blocks
    #[wasm_bindgen(js_name = "listBlocks")]
    pub fn list_blocks(&self) -> Array {
        let arr = Array::new();

        list_registered_blocks().iter().for_each(|block| {
            let desc = JsBlockDesc {
                name: block.name.clone(),
                dis: block.dis.clone(),
                lib: block.library.clone(),
                ver: block.ver.clone(),
                category: block.category.clone(),
                doc: block.doc.clone(),
                implementation: block.implementation.to_string(),

                inputs: block
                    .inputs
                    .iter()
                    .map(|input| JsBlockPin {
                        name: input.name.clone(),
                        kind: input.kind.to_string(),
                    })
                    .collect(),

                outputs: block
                    .outputs
                    .iter()
                    .map(|output| JsBlockPin {
                        name: output.name.clone(),
                        kind: output.kind.to_string(),
                    })
                    .collect(),

                run_condition: block.run_condition.clone().map(|cond| cond.to_string()),
            };

            if let Ok(desc) = serde_wasm_bindgen::to_value(&desc) {
                arr.push(&desc);
            }
        });

        arr
    }

    /// Register a new JS block in the registry
    /// The block is described by a JsBlockDesc object
    ///
    /// # Arguments
    /// * `desc` - The description of the block
    /// * `func` - Optional function that implements the block
    /// 		  logic. If not provided, the block would do nothing.
    ///
    /// # Returns
    /// The name of the block
    ///  
    #[wasm_bindgen(js_name = "registerBlock")]
    pub fn register_block(
        &mut self,
        desc: JsValue,
        func: Option<js_sys::Function>,
    ) -> Result<String, String> {
        let desc: JsBlockDesc =
            serde_wasm_bindgen::from_value(desc).map_err(|err| err.to_string())?;

        let name = desc.name.clone();
        let lib = desc.lib.clone();

        register_block_desc(&desc.into()).map_err(|err| err.to_string())?;

        if let Some(func) = func {
            unsafe {
                JS_FNS
                    .entry(lib)
                    .or_insert(Default::default())
                    .insert(name.clone(), func);
            }
        }

        Ok(name)
    }

    #[wasm_bindgen(js_name = "engineCommand")]
    pub fn engine_command(&mut self) -> EngineCommand {
        let (sender, receiver) = mpsc::channel(32);

        let uuid = Uuid::new_v4();
        let engine_sender = self.engine.create_message_channel(uuid, sender);

        EngineCommand::new(uuid, engine_sender, receiver)
    }

    /// Runs the engine asynchronously
    /// After this is called, the engine instance can't be used directly
    /// Instead use the command object to communicate with the engine.
    #[wasm_bindgen]
    pub async fn run(&mut self) {
        self.engine.run().await;
    }
}
