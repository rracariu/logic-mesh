use std::str::FromStr;

use crate::base::program::data::LinkData;
use crate::wasm::types::JsWatchNotification;

use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::base::engine::messages::EngineMessage;
use crate::single_threaded::Messages;

/// Commands a running instance of a Block Engine.
#[wasm_bindgen]
pub struct EngineCommand {
    uuid: Uuid,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
}

#[wasm_bindgen]
impl EngineCommand {
    /// Creates a new instance of an engine command
    pub(super) fn new(uuid: Uuid, sender: Sender<Messages>, receiver: Receiver<Messages>) -> Self {
        Self {
            uuid,
            sender,
            receiver,
        }
    }

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
                    id.map(|id| id.to_string())
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    /// Removes a block instance from the engine
    /// to be immediately unscheduled for execution
    /// and removed from the engine together with all its links.
    ///
    /// # Arguments
    /// * `block_uuid` - The UUID of the block to be removed
    ///
    /// # Returns
    /// The UUID of the removed block
    #[wasm_bindgen(js_name = "removeBlock")]
    pub async fn remove_block(&mut self, block_uuid: String) -> Option<String> {
        if self
            .sender
            .send(EngineMessage::RemoveBlockReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap(),
            ))
            .await
            .is_ok()
        {
            self.receiver.recv().await.and_then(|msg| {
                if let EngineMessage::RemoveBlockRes(id) = msg {
                    id.map(|id| id.to_string())
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
        target_block_pin_name: String,
    ) -> JsValue {
        if self
            .sender
            .send(EngineMessage::ConnectBlocksReq(
                self.uuid,
                LinkData {
                    source_block_uuid,
                    target_block_uuid,
                    source_block_pin_name,
                    target_block_pin_name,
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

    /// Get the current running engine program.
    /// The program contains the scheduled blocks, their properties, and their links.
    #[wasm_bindgen(js_name = "getProgram")]
    pub async fn get_program(&mut self) -> JsValue {
        if self
            .sender
            .send(EngineMessage::GetCurrentProgramReq(self.uuid))
            .await
            .is_ok()
        {
            self.receiver
                .recv()
                .await
                .and_then(|msg| {
                    if let EngineMessage::GetCurrentProgramRes(_, data) = msg {
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

    /// Creates a watch on block changes
    #[wasm_bindgen(js_name = "createWatch")]
    pub async fn create_watch(&mut self, callback: &js_sys::Function) -> JsValue {
        let (sender, mut receiver) = mpsc::channel(32);

        if self
            .sender
            .send(EngineMessage::WatchBlockSubReq(self.uuid, sender.clone()))
            .await
            .is_ok()
        {
            loop {
                let _ = receiver.recv().await.and_then(|msg| {
                    let js_res = serde_wasm_bindgen::to_value::<JsWatchNotification>(&msg.into());
                    if let Ok(js_res) = js_res {
                        let _ = callback.call1(&JsValue::NULL, &js_res);
                    }
                    None::<JsValue>
                });
            }
        } else {
            JsValue::UNDEFINED
        }
    }
}
