// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::panic;
use std::str::FromStr;

use crate::base::program::data::LinkData;
use crate::blocks::registry::BLOCKS;
use crate::single_threaded::{Messages, SingleThreadedEngine};
use js_sys::Array;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::base::engine::messages::EngineMessage;
use crate::base::engine::Engine;

/// Block field properties, inputs or output
#[derive(Default, Serialize, Deserialize)]
pub struct JsBlockPin {
    pub name: String,
    pub kind: String,
}

/// Block description as a simple struct
#[derive(Default, Serialize, Deserialize)]
pub struct JsBlockDesc {
    pub name: String,
    pub lib: String,
    pub category: String,
    pub doc: String,
    pub inputs: Vec<JsBlockPin>,
    pub outputs: Vec<JsBlockPin>,
}

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
        panic::set_hook(Box::new(console_error_panic_hook::hook));

        Self {
            engine: SingleThreadedEngine::default(),
        }
    }

    /// Lists all available blocks
    #[wasm_bindgen(js_name = "listBlocks")]
    pub fn list_blocks(&self) -> Array {
        let arr = Array::new();

        let blocks = BLOCKS.lock().expect("Failed to lock blocks registry");
        blocks.iter().for_each(|(_, block)| {
            let desc = JsBlockDesc {
                name: block.desc.name.clone(),
                lib: block.desc.library.clone(),
                category: block.desc.category.clone(),
                doc: block.desc.doc.clone(),
                inputs: block
                    .desc
                    .inputs
                    .iter()
                    .map(|input| JsBlockPin {
                        name: input.name.clone(),
                        kind: input.kind.to_string(),
                    })
                    .collect(),
                outputs: block
                    .desc
                    .outputs
                    .iter()
                    .map(|output| JsBlockPin {
                        name: output.name.clone(),
                        kind: output.kind.to_string(),
                    })
                    .collect(),
            };

            if let Ok(desc) = serde_wasm_bindgen::to_value(&desc) {
                arr.push(&desc);
            }
        });

        arr
    }

    #[wasm_bindgen(js_name = "engineCommand")]
    pub fn engine_command(&mut self) -> EngineCommand {
        let (sender, receiver) = mpsc::channel(32);

        let uuid = Uuid::new_v4();
        let engine_sender = self.engine.create_message_channel(uuid, sender);

        EngineCommand {
            uuid,
            sender: engine_sender,
            receiver,
        }
    }

    /// Runs the engine asynchronously
    /// After this is called, the engine instance can't be used directly
    /// Instead use the command object to communicate with the engine.
    #[wasm_bindgen]
    pub async fn run(&mut self) {
        self.engine.run().await;
    }
}

/// Commands a running instance of a Block Engine.
#[wasm_bindgen]
pub struct EngineCommand {
    uuid: Uuid,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
}

#[wasm_bindgen]
impl EngineCommand {
    /// Adds a block instance to the engine
    /// to be immediately scheduled for execution
    #[wasm_bindgen(js_name = "addBlock")]
    pub async fn add_block(&mut self, block_name: String) -> Option<String> {
        if self
            .sender
            .send(EngineMessage::AddBlockReq(self.uuid, block_name))
            .await
            .is_ok()
        {
            self.receiver.recv().await.and_then(|msg| {
                if let EngineMessage::AddBlockRes(id) = msg {
                    Some(id.to_string())
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    /// Creates a link between two blocks
    ///
    /// # Arguments
    /// * `source_block_uuid` - The UUID of the source block
    /// * `target_block_uuid` - The UUID of the target block
    /// * `source_block_pin_name` - The name of the output pin of the source block
    /// * `target_block_input_name` - The name of the input pin of the target block
    ///
    /// # Returns
    /// A `LinkData` object with the following properties:
    /// * `source_block_uuid` - The UUID of the source block
    /// * `target_block_uuid` - The UUID of the target block
    /// * `source_block_pin_name` - The name of the output pin of the source block
    /// * `target_block_input_name` - The name of the input pin of the target block
    ///
    #[wasm_bindgen(js_name = "createLink")]
    pub async fn create_link(
        &mut self,
        source_block_uuid: String,
        target_block_uuid: String,
        source_block_pin_name: String,
        target_block_input_name: String,
    ) -> JsValue {
        if self
            .sender
            .send(EngineMessage::ConnectBlocksReq(
                self.uuid,
                LinkData {
                    source_block_uuid,
                    target_block_uuid,
                    source_block_pin_name,
                    target_block_input_name,
                },
            ))
            .await
            .is_ok()
        {
            self.receiver
                .recv()
                .await
                .and_then(|msg| {
                    if let EngineMessage::ConnectBlocksRes(_, Ok(data)) = msg {
                        serde_wasm_bindgen::to_value(&data).ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(JsValue::UNDEFINED)
        } else {
            JsValue::UNDEFINED
        }
    }

    /// Inspects the current state of a block
    #[wasm_bindgen(js_name = "inspectBlock")]
    pub async fn inspect_block(&mut self, block_uuid: String) -> JsValue {
        if self
            .sender
            .send(EngineMessage::InspectBlockReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap(),
            ))
            .await
            .is_ok()
        {
            self.receiver
                .recv()
                .await
                .and_then(|msg| {
                    if let EngineMessage::InspectBlockRes(_, data) = msg {
                        serde_wasm_bindgen::to_value(&data).ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(JsValue::UNDEFINED)
        } else {
            JsValue::UNDEFINED
        }
    }
}
