// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::base::program::data::LinkData;

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
pub struct BlockParam {
    pub id: String,
    pub name: String,
    pub library: String,
    pub inputs: BTreeMap<String, BlockInputData>,
    pub outputs: BTreeMap<String, BlockOutputData>,
}

#[derive(Debug, Clone)]
pub enum ChangeSource {
    Input(String, Value),
    Output(String, Value),
}

#[derive(Debug, Clone)]
pub struct WatchMessage {
    pub changes: BTreeMap<String, ChangeSource>,
}

/// Messages that engine accepts
#[derive(Debug, Clone)]
pub enum EngineMessage<WatchEventSender: Clone> {
    AddBlockReq(Uuid, String),
    AddBlockRes(Uuid),

    RemoveBlockReq(Uuid, Uuid),
    RemoveBlockRes(Uuid),

    WatchBlockSubReq(Uuid, BTreeSet<String>, WatchEventSender),
    WatchBlockSubRes(Result<Uuid, &'static str>),

    WatchBlockUnsub(Uuid, BTreeSet<String>),
    WatchBlockUnsubRes(Result<Uuid, &'static str>),

    InspectBlockReq(Uuid, Uuid),
    InspectBlockRes(Uuid, Option<BlockParam>),

    ConnectBlocksReq(Uuid, LinkData),
    ConnectBlocksRes(Uuid, Result<LinkData, String>),

    Shutdown,
}
