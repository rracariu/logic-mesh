// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::base::program::data::{BlockData, LinkData};

/// Block input properties
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BlockInputData {
    pub kind: String,
    pub val: Value,
}

/// Block output properties
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BlockOutputData {
    pub kind: String,
    pub val: Value,
}

/// Block definition
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BlockDefinition {
    pub id: String,
    pub name: String,
    pub library: String,
    pub inputs: BTreeMap<String, BlockInputData>,
    pub outputs: BTreeMap<String, BlockOutputData>,
}

/// Defines the source of a change
#[derive(Debug, Clone)]
pub enum ChangeSource {
    Input(String, Value),
    Output(String, Value),
}

/// A notification message for a block change
#[derive(Debug, Clone)]
pub struct WatchMessage {
    pub block_id: Uuid,
    pub changes: HashMap<String, ChangeSource>,
}

/// Messages that engine accepts
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
    GetCurrentProgramRes(Result<(Vec<BlockData>, Vec<LinkData>), String>),

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
