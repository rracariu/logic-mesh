// Copyright (c) 2022-2024, Radu Racariu.

use std::str::FromStr;

use crate::base::program::data::LinkData;
use crate::wasm::types::JsWatchNotification;

use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

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
    pub async fn add_block(
        &mut self,
        block_name: String,
        block_uuid: Option<String>,
        lib: Option<String>,
    ) -> Result<String, String> {
        match self
            .sender
            .send(EngineMessage::AddBlockReq(
                self.uuid, block_name, block_uuid, lib,
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::AddBlockRes(data) => data.map(|ok| ok.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },

            Err(_) => Err("Failed to send message".to_string()),
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
    pub async fn remove_block(&mut self, block_uuid: String) -> Result<String, String> {
        match self
            .sender
            .send(EngineMessage::RemoveBlockReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap_or_default(),
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::RemoveBlockRes(data) => data.map(|ok| ok.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },

            Err(_) => Err("Failed to send message".to_string()),
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
    ) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::ConnectBlocksReq(
                self.uuid,
                LinkData {
                    id: None,
                    source_block_uuid,
                    target_block_uuid,
                    source_block_pin_name,
                    target_block_pin_name,
                },
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::ConnectBlocksRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },

            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Removes a link between two blocks
    ///
    /// # Arguments
    /// * `link_uuid` - The UUID of the link to be removed
    ///
    /// # Returns
    /// True if the link was removed, false otherwise
    #[wasm_bindgen(js_name = "removeLink")]
    pub async fn remove_link(&mut self, link_uuid: String) -> Result<bool, String> {
        match self
            .sender
            .send(EngineMessage::RemoveLinkReq(
                self.uuid,
                Uuid::from_str(&link_uuid).unwrap_or_default(),
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::RemoveLinkRes(data) => data,
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Writes the given block out with a value
    ///
    /// # Arguments
    /// * `block_uuid` - The UUID of the block to write to
    /// * `output_name` - The name of the output to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    /// The block data
    #[wasm_bindgen(js_name = "writeBlockOutput")]
    pub async fn write_block_output(
        &mut self,
        block_uuid: String,
        output_name: String,
        value: JsValue,
    ) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::WriteBlockOutputReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap_or_default(),
                output_name,
                serde_wasm_bindgen::from_value(value).unwrap_or_default(),
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::WriteBlockOutputRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },

            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Writes the given block input with a value
    ///
    /// # Arguments
    /// * `block_uuid` - The UUID of the block to write to
    /// * `input_name` - The name of the input to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    /// The previous value of the input
    #[wasm_bindgen(js_name = "writeBlockInput")]
    pub async fn write_block_input(
        &mut self,
        block_uuid: String,
        input_name: String,
        value: JsValue,
    ) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::WriteBlockInputReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap_or_default(),
                input_name,
                serde_wasm_bindgen::from_value(value).unwrap_or_default(),
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::WriteBlockInputRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },

            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Get the current running engine program.
    /// The program contains the scheduled blocks, their properties, and their links.
    #[wasm_bindgen(js_name = "getProgram")]
    pub async fn get_program(&mut self) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::GetCurrentProgramReq(self.uuid))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::GetCurrentProgramRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Inspects the current state of a block
    #[wasm_bindgen(js_name = "inspectBlock")]
    pub async fn inspect_block(&mut self, block_uuid: String) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::InspectBlockReq(
                self.uuid,
                Uuid::from_str(&block_uuid).unwrap_or_default(),
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::InspectBlockRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Evaluates a block by name
    /// This will create a block instance and execute it.
    ///
    /// # Arguments
    /// * `block_name` - The name of the block to evaluate
    /// * `inputs` - The input values to the block
    /// * `lib` - Optional, the library to load the block from, defaults to "core"
    ///
    /// # Returns
    /// A list of values representing the outputs of the block
    #[wasm_bindgen(js_name = "evalBlock")]
    pub async fn eval_block(
        &mut self,
        block_name: String,
        inputs: Vec<JsValue>,
        lib: Option<String>,
    ) -> Result<JsValue, String> {
        match self
            .sender
            .send(EngineMessage::EvaluateBlockReq(
                self.uuid,
                block_name,
                inputs
                    .into_iter()
                    .map(|v| serde_wasm_bindgen::from_value(v).unwrap_or_default())
                    .collect(),
                lib,
            ))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(res) => match res {
                    EngineMessage::EvaluateBlockRes(data) => data
                        .map(|ok| serde_wasm_bindgen::to_value(&ok))?
                        .map_err(|err| err.to_string()),
                    _ => Err("Invalid response".to_string()),
                },
                None => Err("Failed to receive message".to_string()),
            },
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Creates a watch on block changes
    #[wasm_bindgen(js_name = "createWatch")]
    pub async fn create_watch(&mut self, callback: &js_sys::Function) -> Result<(), String> {
        let (sender, mut receiver) = mpsc::channel(32);

        match self
            .sender
            .send(EngineMessage::WatchBlockSubReq(self.uuid, sender.clone()))
            .await
        {
            Ok(_) => loop {
                if let Some(msg) = receiver.recv().await {
                    match serde_wasm_bindgen::to_value::<JsWatchNotification>(&msg.into())
                        .map_err(|err| format!("Failed to deserialize watch message: {:?}", err))
                        .and_then(|js_res| {
                            callback
                                .call1(&JsValue::NULL, &js_res)
                                .map_err(|err| format!("Failed to call watch callback: {:?}", err))
                        }) {
                        Ok(_) => (),
                        Err(err) => log::debug!(target: "create_watch", "{err}"),
                    }
                }
            },
            Err(_) => return Err("Failed to send message".to_string()),
        }
    }

    /// Pauses the execution of the engine
    /// If the engine is already paused, this does nothing
    #[wasm_bindgen(js_name = "pauseExecution")]
    pub async fn pause_execution(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Pause)
            .await
            .map_err(|err| err.to_string())
    }

    /// Resumes the execution of the engine
    /// If the engine is not paused, this does nothing
    #[wasm_bindgen(js_name = "resumeExecution")]
    pub async fn resume_execution(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Resume)
            .await
            .map_err(|err| err.to_string())
    }

    /// Resets the engine state, clears all blocks and links
    #[wasm_bindgen(js_name = "resetEngine")]
    pub async fn reset_engine(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Reset)
            .await
            .map_err(|err| err.to_string())
    }

    /// Stop the engine's execution
    #[wasm_bindgen(js_name = "stopEngine")]
    pub async fn stop_engine(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Shutdown)
            .await
            .map_err(|err| err.to_string())
    }
}
