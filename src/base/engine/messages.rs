// Copyright (c) 2022-2023, Radu Racariu.

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
    pub kind: String,
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
    pub kind: String,
    pub val: Value,
}

/// Block definition.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BlockDefinition {
    pub id: String,
    pub name: String,
    pub library: String,
    pub inputs: BTreeMap<String, BlockInputData>,
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
    Input(String, Value),
    Output(String, Value),
}

/// A notification message for a block change.
#[derive(Debug, Clone)]
pub struct WatchMessage {
    pub block_id: Uuid,
    pub changes: HashMap<String, ChangeSource>,
    /// Block's operational state at the time the notification was sent.
    /// Carries fault propagation visibility to the UI.
    pub state: BlockState,
}

/// Messages that the engine accepts.
#[derive(Debug, Clone)]
pub enum EngineMessage<WatchEventSender: Clone> {
    AddBlockReq(Uuid, String, Option<String>, Option<String>),
    AddBlockRes(Result<Uuid, String>),

    RemoveBlockReq(Uuid, Uuid),
    RemoveBlockRes(Result<Uuid, String>),

    WatchBlockSubReq(Uuid, WatchEventSender),
    WatchBlockSubRes(Result<Uuid, &'static str>),

    WriteBlockOutputReq(Uuid, Uuid, String, Value),
    WriteBlockOutputRes(Result<Value, String>),

    WriteBlockInputReq(Uuid, Uuid, String, Value),
    WriteBlockInputRes(Result<Option<Value>, String>),

    WatchBlockUnsubReq(Uuid),
    WatchBlockUnsubRes(Result<Uuid, &'static str>),

    GetCurrentProgramReq(Uuid),
    GetCurrentProgramRes(Result<Program, String>),

    /// Atomically load a full `Program` (blocks, links, pin values, UI
    /// metadata) into the engine. Replaces the multi-call JS chain of
    /// `addBlock` + `createLink` + `writeBlockInput` per block.
    LoadProgramReq(Uuid, Program),
    LoadProgramRes(Result<(), String>),

    InspectBlockReq(Uuid, Uuid),
    InspectBlockRes(Result<BlockDefinition, String>),

    EvaluateBlockReq(Uuid, String, Vec<Value>, Option<String>),
    EvaluateBlockRes(Result<Vec<Value>, String>),

    ConnectBlocksReq(Uuid, LinkData),
    ConnectBlocksRes(Result<LinkData, String>),

    RemoveLinkReq(Uuid, Uuid),
    RemoveLinkRes(Result<bool, String>),

    Shutdown,
    Pause,
    Resume,
    Reset,
}
