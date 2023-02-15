// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BlockData {
    pub id: String,
    pub name: String,
    pub inputs: BTreeMap<String, BlockInputData>,
    pub output: BlockOutputData,
}

/// Messages that engine accepts
#[derive(Debug, Clone)]
pub enum EngineMessage {
    AddBlock(Uuid, String),
    BlockAdded(Uuid),
    InspectBlock(Uuid, Uuid),
    BlockData(Uuid, BlockData),
}

/// Messages that blocks accepts
#[derive(Debug, Default, Clone)]
pub(crate) enum BlockMessage {
    #[default]
    Nop,
    InspectBlock(Uuid, Uuid),
}
