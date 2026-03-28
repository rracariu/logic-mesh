// Copyright (c) 2022-2026, Radu Racariu.

use crate::base::block::BlockState;
use crate::base::engine::messages::BlockDefinition;
use crate::base::program::data::BlockData;
use crate::base::program::data::LinkData;
use libhaystack::val::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Commands sent from the engine to individual block tasks
/// in the multi-threaded engine.
pub enum BlockCommand {
    /// Inspect a block's state and return its definition.
    Inspect(oneshot::Sender<Result<BlockDefinition, String>>),

    /// Write a value to a block's output pin.
    WriteOutput(String, Value, oneshot::Sender<Result<Value, String>>),

    /// Write a value to a block's input pin.
    WriteInput(
        String,
        Value,
        oneshot::Sender<Result<Option<Value>, String>>,
    ),

    /// Get a clone of the writer (Sender) for a given input pin.
    GetInputWriter(String, oneshot::Sender<Result<mpsc::Sender<Value>, String>>),

    /// Add a link from this block's output to a target input's writer.
    AddOutputLink {
        output_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        writer: mpsc::Sender<Value>,
        reply: oneshot::Sender<Result<Uuid, String>>,
    },

    /// Add a link from this block's input to a target input's writer.
    AddInputLink {
        input_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        writer: mpsc::Sender<Value>,
        reply: oneshot::Sender<Result<Uuid, String>>,
    },

    /// Remove a link by ID.
    RemoveLink(Uuid, oneshot::Sender<bool>),

    /// Remove all links targeting a specific block.
    RemoveTargetBlockLinks(Uuid),

    /// Get the block's current data for program serialization.
    GetBlockData(oneshot::Sender<(BlockData, Vec<LinkData>)>),

    /// Set the block's state (e.g., Terminated).
    SetState(BlockState),
}
