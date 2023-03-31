// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::panic;
use std::str::FromStr;

use crate::blocks::registry::BLOCKS;
use js_sys::Array;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::base::engine_messages::{EngineMessage, LinkData};
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
    pub outputs: Vec<BlockFieldProp>,
}

/// Block properties
#[derive(Default, Serialize, Deserialize)]
pub struct LinkProperties {
    pub source_block_uuid: String,
    pub target_block_uuid: String,
    pub target_block_input_name: String,
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
        panic::set_hook(Box::new(console_error_panic_hook::hook));

        Self {
            engine: Engine::default(),
        }
    }

    /// Lists all available blocks
    #[wasm_bindgen(js_name = "listBlocks")]
    pub fn list_blocks(&self) -> Array {
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
                outputs: desc
                    .outputs
                    .iter()
                    .map(|output| BlockFieldProp {
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
        let engine_sender = self.engine.message_handles(uuid, sender);

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
    sender: Sender<EngineMessage>,
    receiver: Receiver<EngineMessage>,
}

#[wasm_bindgen]
impl EngineCommand {
    /// Adds a block instance to the engine
    /// to be immediately scheduled for execution
    #[wasm_bindgen(js_name = "addBlock")]
    pub async fn add_block(&mut self, block_name: String) -> Option<String> {
        if self
            .sender
            .send(EngineMessage::AddBlock(self.uuid, block_name))
            .await
            .is_ok()
        {
            self.receiver.recv().await.and_then(|msg| {
                if let EngineMessage::BlockAdded(id) = msg {
                    Some(id.to_string())
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    /// Inspects the current state of a block
    #[wasm_bindgen(js_name = "createLink")]
    pub async fn crate_link(
        &mut self,
        source_block_uuid: String,
        target_block_uuid: String,
        target_block_input_name: String,
    ) -> JsValue {
        if self
            .sender
            .send(EngineMessage::ConnectBlocksReq(
                self.uuid,
                LinkData {
                    source_block_uuid: Uuid::from_str(&source_block_uuid).unwrap(),
                    target_block_uuid: Uuid::from_str(&target_block_uuid).unwrap(),
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
                    if let EngineMessage::ConnectBlocksRes(_, Some(data)) = msg {
                        serde_wasm_bindgen::to_value(&LinkProperties {
                            source_block_uuid: data.source_block_uuid.to_string(),
                            target_block_uuid: data.target_block_uuid.to_string(),
                            target_block_input_name: data.target_block_input_name,
                        })
                        .ok()
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
                    if let EngineMessage::FoundBlockRes(_, data) = msg {
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
