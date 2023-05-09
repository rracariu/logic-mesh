// Copyright (c) 2022-2023, IntriSemantics Corp.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProgramMeta {
    pub name: String,
    pub libs: Vec<String>,
    pub ver: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LinkData {
    pub source_block_uuid: String,
    pub target_block_uuid: String,
    pub source_block_pin_name: String,
    pub target_block_input_name: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BlockData {
    pub id: String,
    pub name: String,
    pub lib: String,
    pub ver: String,
}