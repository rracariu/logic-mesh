// Copyright (c) 2022-2023, Radu Racariu.

//! Block engine exposed to JavaScript.

use crate::blocks::registry::{list_registered_blocks, register_block_desc};
use crate::blocks::utils::set_sleep_dur;
use crate::single_threaded::SingleThreadedEngine;
use crate::wasm::engine_command::EngineCommand;
use crate::wasm::js_block::JS_FNS;
use crate::wasm::types::{JsBlockDesc, JsBlockPin};
use js_sys::Array;

use tokio::sync::mpsc;
use uuid::Uuid;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::base::engine::Engine;

/// Controls the execution of blocks.
///
/// Loads programs and enables inspection and debugging
/// of blocks and their inputs and outputs.
#[wasm_bindgen]
pub struct BlocksEngine {
    engine: SingleThreadedEngine,
}

#[wasm_bindgen]
impl BlocksEngine {
    /// Creates a new engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new(sleep_duration: Option<u64>) -> Self {
        if let Some(sleep_duration) = sleep_duration {
            set_sleep_dur(sleep_duration);
        }

        Self {
            engine: SingleThreadedEngine::default(),
        }
    }

    /// Lists all available blocks.
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

    /// Registers a new JS block in the registry and returns its name.
    ///
    /// `desc` is a [`JsBlockDesc`] describing the block. `func`, if provided,
    /// is the JavaScript function that implements the block logic — without it
    /// the block is a no-op.
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
            JS_FNS.with_borrow_mut(|reg| {
                reg.entry(lib).or_default().insert(name.clone(), func);
            });
        }

        Ok(name)
    }

    /// Returns a new [`EngineCommand`] handle for sending commands.
    #[wasm_bindgen(js_name = "engineCommand")]
    pub fn engine_command(&mut self) -> EngineCommand {
        let (sender, receiver) = mpsc::channel(32);

        let uuid = Uuid::new_v4();
        let engine_sender = self.engine.create_message_channel(uuid, sender);

        EngineCommand::new(uuid, engine_sender, receiver)
    }

    /// Runs the engine asynchronously.
    ///
    /// After this is called, the engine instance can't be used directly —
    /// use the command object to communicate with the engine instead.
    #[wasm_bindgen]
    pub async fn run(&mut self) {
        self.engine.run().await;
    }
}
