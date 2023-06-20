// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

use crate::base::engine::messages::{ChangeSource, WatchMessage};

/// Block field properties, inputs or output
#[derive(Default, Serialize, Deserialize)]
pub struct JsBlockPin {
    pub name: String,
    pub kind: String,
}

/// Block description as a simple struct
#[derive(Default, Serialize, Deserialize)]
pub struct JsBlockDesc {
    pub name: String,
    pub lib: String,
    pub category: String,
    pub doc: String,
    pub inputs: Vec<JsBlockPin>,
    pub outputs: Vec<JsBlockPin>,
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
