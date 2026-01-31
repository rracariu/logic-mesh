// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

use crate::base::{
    block::{BlockDesc, BlockPin, desc::BlockImplementation},
    engine::messages::{ChangeSource, WatchMessage},
};

/// Block field properties, inputs or output
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct JsBlockPin {
    pub name: String,
    pub kind: String,
}

/// Block description as a simple struct
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsBlockDesc {
    pub name: String,
    pub dis: String,
    pub lib: String,
    pub ver: String,
    pub category: String,
    pub doc: String,
    pub implementation: String,
    pub inputs: Vec<JsBlockPin>,
    pub outputs: Vec<JsBlockPin>,
    pub run_condition: Option<String>,
}

impl From<JsBlockDesc> for BlockDesc {
    fn from(desc: JsBlockDesc) -> Self {
        Self {
            name: desc.name,
            dis: desc.dis,
            library: desc.lib,
            ver: desc.ver,
            category: desc.category,
            doc: desc.doc,
            implementation: BlockImplementation::External,

            inputs: desc
                .inputs
                .into_iter()
                .map(|pin| BlockPin {
                    name: pin.name,
                    kind: pin.kind.as_str().try_into().unwrap_or_default(),
                })
                .collect(),

            outputs: desc
                .outputs
                .into_iter()
                .map(|pin| BlockPin {
                    name: pin.name,
                    kind: pin.kind.as_str().try_into().unwrap_or_default(),
                })
                .collect(),

            run_condition: desc
                .run_condition
                .map(|cond| cond.as_str().try_into().unwrap_or_default()),
        }
    }
}

impl From<BlockDesc> for JsBlockDesc {
    fn from(desc: BlockDesc) -> Self {
        Self {
            name: desc.name,
            dis: desc.dis,
            lib: desc.library,
            ver: desc.ver,
            category: desc.category,
            doc: desc.doc,
            implementation: desc.implementation.to_string(),

            inputs: desc
                .inputs
                .into_iter()
                .map(|pin| JsBlockPin {
                    name: pin.name,
                    kind: pin.kind.to_string(),
                })
                .collect(),

            outputs: desc
                .outputs
                .into_iter()
                .map(|pin| JsBlockPin {
                    name: pin.name,
                    kind: pin.kind.to_string(),
                })
                .collect(),

            run_condition: desc.run_condition.map(|cond| cond.to_string()),
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct JsWatchNotification {
    pub id: String,
    pub changes: Vec<JsWatchChange>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct JsWatchChange {
    pub name: String,
    pub source: String,
    pub value: Value,
}

impl From<WatchMessage> for JsWatchNotification {
    fn from(msg: WatchMessage) -> Self {
        let block_id = msg.block_id.to_string();
        let changes = msg
            .changes
            .into_iter()
            .map(|(name, source)| JsWatchChange {
                name,
                source: match source {
                    ChangeSource::Input(_, _) => "input".to_string(),
                    ChangeSource::Output(_, _) => "output".to_string(),
                },
                value: match source {
                    ChangeSource::Input(_, v) => v,
                    ChangeSource::Output(_, v) => v,
                },
            })
            .collect();

        Self {
            id: block_id,
            changes,
        }
    }
}
