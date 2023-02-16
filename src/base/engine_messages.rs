// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
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

#[derive(Debug, Default, Clone)]
pub struct LinkData {
    pub source_block_uuid: Uuid,
    pub target_block_uuid: Uuid,
    pub target_block_input_name: String,
}

/// Messages that engine accepts
#[derive(Debug, Clone)]
pub enum EngineMessage {
    AddBlock(Uuid, String),
    BlockAdded(Uuid),

    InspectBlock(Uuid, Uuid),
    FoundBlockData(Uuid, BlockData),

    ConnectBlocks(Uuid, LinkData),
    FoundBlockInputWriter(Uuid, LinkData, Sender<Value>),
    LinkCreated(Uuid, Option<LinkData>),
}

/// Messages that blocks accepts
#[derive(Debug, Default, Clone)]
pub(crate) enum BlockMessage {
    #[default]
    Nop,
    InspectBlock(Uuid, Uuid),
    GetInputWriter(Uuid, LinkData),
    ConnectBlocks(Uuid, LinkData, Sender<Value>),
}
