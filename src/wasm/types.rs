// Copyright (c) 2022-2023, Radu Racariu.

//! TypeScript-facing data types.

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

use crate::base::{
    block::{BlockDesc, BlockPin, desc::BlockImplementation},
    engine::messages::{ChangeSource, WatchMessage},
};

/// Block field properties, inputs or output
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct JsBlockPin {
    /// Pin name.
    pub name: String,
    /// Haystack kind as a string.
    pub kind: String,
}

/// Block description as a simple struct
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsBlockDesc {
    /// Block type name.
    pub name: String,
    /// Display name.
    pub dis: String,
    /// Library the block belongs to.
    pub lib: String,
    /// Semantic version.
    pub ver: String,
    /// Functional category.
    pub category: String,
    /// Documentation string.
    pub doc: String,
    /// Implementation kind (native or external).
    pub implementation: String,
    /// Input pin descriptors.
    pub inputs: Vec<JsBlockPin>,
    /// Output pin descriptors.
    pub outputs: Vec<JsBlockPin>,
    /// Optional run condition expression.
    pub run_condition: Option<String>,
}

impl TryFrom<JsBlockDesc> for BlockDesc {
    type Error = String;

    /// Strict conversion: an unrecognized pin kind, run condition, or
    /// implementation is an error, not a silent default. Kinds are the
    /// lowercase haystack names (`"number"`, `"str"`, …) — defaulting a
    /// typo to `Null` used to register an accepts-anything pin that
    /// behaved nothing like the declared type. The implementation must
    /// be `"external"`: this conversion serves JS block registration,
    /// which cannot produce a native block.
    fn try_from(desc: JsBlockDesc) -> Result<Self, Self::Error> {
        let pin = |pin: JsBlockPin| -> Result<BlockPin, String> {
            let kind = pin
                .kind
                .as_str()
                .try_into()
                .map_err(|err| format!("pin '{}': {err}", pin.name))?;
            Ok(BlockPin {
                name: pin.name,
                kind,
            })
        };

        // JS registration only ever produces external blocks (the JS
        // function IS the implementation), so anything else in the
        // field is a caller mistake — likely a native desc copied from
        // `listBlocks` — and silently registering it as external would
        // hide that.
        let implementation = match desc.implementation.as_str() {
            "external" => BlockImplementation::External,
            other => {
                return Err(format!(
                    "field 'implementation': a JS-registered block must be \
                     'external', got '{other}'"
                ));
            }
        };

        Ok(Self {
            name: desc.name,
            dis: desc.dis,
            library: desc.lib,
            ver: desc.ver,
            category: desc.category,
            doc: desc.doc,
            implementation,

            inputs: desc
                .inputs
                .into_iter()
                .map(pin)
                .collect::<Result<Vec<_>, _>>()?,

            outputs: desc
                .outputs
                .into_iter()
                .map(pin)
                .collect::<Result<Vec<_>, _>>()?,

            run_condition: desc
                .run_condition
                .map(|cond| {
                    cond.as_str()
                        .try_into()
                        .map_err(|err| format!("field 'runCondition': {err}"))
                })
                .transpose()?,
        })
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

/// A watch notification sent to JavaScript when a block's state changes.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsWatchNotification {
    /// Block UUID as a string.
    pub id: String,
    /// Pin changes since the last notification.
    pub changes: Vec<JsWatchChange>,
    /// Block's operational state at the time of the notification
    /// (`running | fault | disabled | terminated`).
    pub state: String,
    /// Fault reason when `state == "fault"`, else [`None`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault_reason: Option<String>,
}

/// A single pin value change within a [`JsWatchNotification`].
#[derive(Default, Serialize, Deserialize)]
pub struct JsWatchChange {
    /// Pin name that changed.
    pub name: String,
    /// `"input"` or `"output"`.
    pub source: String,
    /// New pin value.
    pub value: Value,
}

impl From<WatchMessage> for JsWatchNotification {
    fn from(msg: WatchMessage) -> Self {
        let block_id = msg.block_id.to_string();
        let state = msg.state.label().to_string();
        let fault_reason = msg.state.fault_reason().map(|s| s.to_string());
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
            state,
            fault_reason,
        }
    }
}
