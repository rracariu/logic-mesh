// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::panic;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, Receiver, Sender};
use uuid::Uuid;
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
        panic::set_hook(Box::new(console_error_panic_hook::hook));

        Self {
            engine: Engine::default(),
        }
    }

    #[wasm_bindgen(js_name = "engineCommand")]
    pub fn engine_command(&mut self) -> EngineCommand {
        let (sender, receiver) = mpsc::channel(32);
        let engine_sender = self.engine.message_handles(Uuid::new_v4(), sender);

        EngineCommand {
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
    sender: Sender<String>,
    receiver: Receiver<String>,
}

#[wasm_bindgen]
impl EngineCommand {
    /// Adds a block instance to the engine
    /// to be immediately scheduled for execution
    #[wasm_bindgen(js_name = "addBlock")]
    pub async fn add_block(&mut self, block_name: String) -> Option<String> {
        if self.sender.send(block_name).await.is_ok() {
            self.receiver.recv().await
        } else {
            None
        }
    }
}
