// Copyright (c) 2022-2023, Radu Racariu.

//! Engine message types.

use std::collections::{BTreeMap, HashMap};

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::base::block::BlockState;
use crate::base::program::{Program, data::LinkData};

/// Block input properties.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInputData {
    /// Haystack type kind.
    pub kind: String,
    /// Current value.
    pub val: Value,
    /// True if this input is wired up (i.e. its `connection_count > 0`).
    /// Surfaced so `save_program` can round-trip pin connectedness
    /// alongside the value.
    #[serde(default)]
    pub is_connected: bool,
}

/// Block output properties.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BlockOutputData {
    /// Haystack type kind.
    pub kind: String,
    /// Current value.
    pub val: Value,
}

/// Block definition.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BlockDefinition {
    /// Block instance UUID.
    pub id: String,
    /// Block type name.
    pub name: String,
    /// Library the block belongs to.
    pub library: String,
    /// Input pins keyed by name.
    pub inputs: BTreeMap<String, BlockInputData>,
    /// Output pins keyed by name.
    pub outputs: BTreeMap<String, BlockOutputData>,
    /// Short label for the block's operational state
    /// (`running | fault | disabled | terminated`).
    #[serde(default)]
    pub state: String,
    /// Fault reason when `state == "fault"`, else `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fault_reason: Option<String>,
}

/// The source of a change.
#[derive(Debug, Clone)]
pub enum ChangeSource {
    /// Change originated from an input pin.
    Input(String, Value),
    /// Change originated from an output pin.
    Output(String, Value),
}

/// A notification message for a block change.
#[derive(Debug, Clone)]
pub struct WatchMessage {
    /// UUID of the block that changed.
    pub block_id: Uuid,
    /// Changed pins keyed by name.
    pub changes: HashMap<String, ChangeSource>,
    /// Block's operational state at the time the notification was sent.
    /// Carries fault propagation visibility to the UI.
    pub state: BlockState,
}

/// Messages that the engine accepts.
#[derive(Debug, Clone)]
pub enum EngineMessage<WatchEventSender: Clone> {
    /// Request to add a block by name.
    AddBlockReq(Uuid, String, Option<String>, Option<String>),
    /// Response to [`AddBlockReq`](Self::AddBlockReq).
    AddBlockRes(Result<Uuid, String>),

    /// Request to remove a block by UUID.
    RemoveBlockReq(Uuid, Uuid),
    /// Response to [`RemoveBlockReq`](Self::RemoveBlockReq).
    RemoveBlockRes(Result<Uuid, String>),

    /// Subscribe to block change-of-value notifications.
    WatchBlockSubReq(Uuid, WatchEventSender),
    /// Response to [`WatchBlockSubReq`](Self::WatchBlockSubReq).
    WatchBlockSubRes(Result<Uuid, &'static str>),

    /// Write a value to a block's output pin.
    WriteBlockOutputReq(Uuid, Uuid, String, Value),
    /// Response to [`WriteBlockOutputReq`](Self::WriteBlockOutputReq).
    WriteBlockOutputRes(Result<Value, String>),

    /// Write a value to a block's input pin.
    WriteBlockInputReq(Uuid, Uuid, String, Value),
    /// Response to [`WriteBlockInputReq`](Self::WriteBlockInputReq).
    WriteBlockInputRes(Result<Option<Value>, String>),

    /// Unsubscribe from block change notifications.
    WatchBlockUnsubReq(Uuid),
    /// Response to [`WatchBlockUnsubReq`](Self::WatchBlockUnsubReq).
    WatchBlockUnsubRes(Result<Uuid, &'static str>),

    /// Request the current program in save format.
    GetCurrentProgramReq(Uuid),
    /// Response to [`GetCurrentProgramReq`](Self::GetCurrentProgramReq).
    GetCurrentProgramRes(Result<Program, String>),

    /// Atomically load a full `Program` (blocks, links, pin values, UI
    /// metadata) into the engine. Replaces the multi-call JS chain of
    /// `addBlock` + `createLink` + `writeBlockInput` per block.
    LoadProgramReq(Uuid, Program),
    /// Response to [`LoadProgramReq`](Self::LoadProgramReq).
    LoadProgramRes(Result<(), String>),

    /// Request to inspect a block's current state.
    InspectBlockReq(Uuid, Uuid),
    /// Response to [`InspectBlockReq`](Self::InspectBlockReq).
    InspectBlockRes(Result<BlockDefinition, String>),

    /// Evaluate a block by name with given inputs.
    EvaluateBlockReq(Uuid, String, Vec<Value>, Option<String>),
    /// Response to [`EvaluateBlockReq`](Self::EvaluateBlockReq).
    EvaluateBlockRes(Result<Vec<Value>, String>),

    /// Connect two blocks via a link.
    ConnectBlocksReq(Uuid, LinkData),
    /// Response to [`ConnectBlocksReq`](Self::ConnectBlocksReq).
    ConnectBlocksRes(Result<LinkData, String>),

    /// Remove a link by UUID.
    RemoveLinkReq(Uuid, Uuid),
    /// Response to [`RemoveLinkReq`](Self::RemoveLinkReq).
    RemoveLinkRes(Result<bool, String>),

    /// Shut down the engine.
    Shutdown,
    /// Pause block execution.
    Pause,
    /// Resume block execution.
    Resume,
    /// Reset the engine, removing all blocks and links.
    Reset,
}
