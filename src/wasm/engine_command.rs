// Copyright (c) 2022-2024, Radu Racariu.

//! Commands sent from JavaScript to the engine.

use std::str::FromStr;

use crate::base::program::Program;
use crate::base::program::data::LinkData;
use crate::wasm::types::JsWatchNotification;

use tokio::sync::mpsc::{Receiver, Sender, unbounded_channel};
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
    /// Creates a new engine command handle.
    pub(super) fn new(uuid: Uuid, sender: Sender<Messages>, receiver: Receiver<Messages>) -> Self {
        Self {
            uuid,
            sender,
            receiver,
        }
    }

    /// Adds a block instance to the engine, immediately scheduling it
    /// for execution.
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

    /// Removes a block instance and all its links from the engine,
    /// returning the UUID of the removed block.
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

    /// Creates a link between two blocks and returns the resulting
    /// [`LinkData`].
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

    /// Removes a link by UUID, returning `true` if it was found and removed.
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

    /// Writes a value to a block's output pin.
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

    /// Writes a value to a block's input pin, returning the previous value.
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

    /// Atomically load a full program (blocks + links + pin values +
    /// UI metadata) into the engine. Replaces the previous JS-side
    /// `addBlock` + `createLink` + `writeBlockInput` chain. The engine
    /// is expected to be empty (call `resetEngine` first if reloading).
    #[wasm_bindgen(js_name = "loadProgram")]
    pub async fn load_program(&mut self, program: JsValue) -> Result<(), String> {
        let program: Program = serde_wasm_bindgen::from_value(program)
            .map_err(|err| format!("Invalid program payload: {err}"))?;
        match self
            .sender
            .send(EngineMessage::LoadProgramReq(self.uuid, program))
            .await
        {
            Ok(_) => match self.receiver.recv().await {
                Some(EngineMessage::LoadProgramRes(res)) => res,
                Some(_) => Err("Invalid response".to_string()),
                None => Err("Failed to receive message".to_string()),
            },
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Returns the current running engine program in the canonical save
    /// format ([`Program`] serialized as JSON). Round-trips through
    /// `loadProgram` without re-assembly.
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

    /// Inspects the current state of a block.
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

    /// Evaluates a block by name, returning its output values.
    ///
    /// Creates a temporary block instance, feeds it the given `inputs`,
    /// executes it, and returns the outputs. `lib` defaults to `"core"`.
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

    /// Creates a watch on block changes.
    ///
    /// The engine→UI watch channel is **unbounded**. The producer rate is
    /// capped by per-block execution cadence and the payload is tiny
    /// (one [`WatchMessage`](crate::base::engine::messages::WatchMessage)
    /// per changed block per cycle); a bounded channel
    /// (we had 32) was undersized for bursty loads — e.g., dozens of
    /// blocks faulting simultaneously during program load — and silently
    /// dropped fault notifications, which the UI is now load-bearing on
    /// (red ring/edge rendering).
    #[wasm_bindgen(js_name = "createWatch")]
    pub async fn create_watch(&mut self, callback: &js_sys::Function) -> Result<(), String> {
        let (sender, mut receiver) = unbounded_channel();

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
            Err(_) => Err("Failed to send message".to_string()),
        }
    }

    /// Pauses the engine. Does nothing if already paused.
    #[wasm_bindgen(js_name = "pauseExecution")]
    pub async fn pause_execution(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Pause)
            .await
            .map_err(|err| err.to_string())
    }

    /// Resumes the engine. Does nothing if not paused.
    #[wasm_bindgen(js_name = "resumeExecution")]
    pub async fn resume_execution(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Resume)
            .await
            .map_err(|err| err.to_string())
    }

    /// Resets the engine state, clearing all blocks and links.
    #[wasm_bindgen(js_name = "resetEngine")]
    pub async fn reset_engine(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Reset)
            .await
            .map_err(|err| err.to_string())
    }

    /// Stops the engine's execution.
    #[wasm_bindgen(js_name = "stopEngine")]
    pub async fn stop_engine(&mut self) -> Result<(), String> {
        self.sender
            .send(EngineMessage::Shutdown)
            .await
            .map_err(|err| err.to_string())
    }
}
