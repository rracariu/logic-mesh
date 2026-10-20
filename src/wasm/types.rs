// Copyright (c) 2022-2023, Radu Racariu.

//! TypeScript-facing data types.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::base::{
    block::{BlockDesc, BlockImplementation, BlockPin},
    engine::messages::{
        BlockDefinition, BlockInputData, BlockOutputData, ChangeSource, WatchMessage,
    },
    error::BlockDescError,
};

/// The TypeScript types the generated declarations name in place of
/// `any`, through `unchecked_param_type` and `unchecked_return_type`.
/// They are declared by hand in the npm package's `src/index.ts`, next
/// to the wrapper that re-exports the bindings.
///
/// The import makes the declarations of the generated module and of
/// the package's index import each other. That is fine for types, but
/// it makes the built `index.d.ts` an input of the package's `tsc`, so
/// rerunning `tsc` over a finished build fails with TS5055: the package
/// scripts build from a clean `dist/`.
#[wasm_bindgen(typescript_custom_section)]
const TS_IMPORTS: &'static str = r#"
import type {
  BlockDesc,
  BlockInspection,
  BlockNotification,
  EditorMetadata,
  JsBlockFactory,
  JsConnector,
  LinkData,
  LogLevel,
  Position,
  Program,
} from './index.js';
"#;

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
    type Error = BlockDescError;

    /// Strict conversion: an unrecognized pin kind, run condition, or
    /// implementation is an error, not a silent default. Kinds are the
    /// lowercase haystack names (`"number"`, `"str"`, …) — defaulting a
    /// typo to `Null` used to register an accepts-anything pin that
    /// behaved nothing like the declared type. The implementation must
    /// be `"external"`: this conversion serves JS block registration,
    /// which cannot produce a native block.
    fn try_from(desc: JsBlockDesc) -> Result<Self, Self::Error> {
        let pin = |pin: JsBlockPin| -> Result<BlockPin, BlockDescError> {
            match pin.kind.as_str().try_into() {
                Ok(kind) => Ok(BlockPin {
                    name: pin.name,
                    kind,
                }),
                Err(_) => Err(BlockDescError::UnknownPinKind {
                    pin: pin.name,
                    kind: pin.kind,
                }),
            }
        };

        // JS registration only ever produces external blocks (the JS
        // function IS the implementation), so anything else in the
        // field is a caller mistake — likely a native desc copied from
        // `listBlocks` — and silently registering it as external would
        // hide that.
        let implementation = match desc.implementation.as_str() {
            "external" => BlockImplementation::External,
            other => {
                return Err(BlockDescError::Field {
                    field: "implementation",
                    error: Box::new(BlockDescError::NotExternal {
                        implementation: other.to_string(),
                    }),
                });
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
                    cond.parse().map_err(|err| BlockDescError::Field {
                        field: "runCondition",
                        error: Box::new(err),
                    })
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

/// A block's state and pin values, as `inspectBlock` resolves with them
/// — the JavaScript form of a [`BlockDefinition`], with the field names
/// of the other JavaScript types.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsBlockInspection {
    /// Block UUID as a string.
    pub id: String,
    /// Block type name.
    pub name: String,
    /// Library of the block type.
    pub lib: String,
    /// Input pins by name.
    pub inputs: BTreeMap<String, BlockInputData>,
    /// Output pins by name.
    pub outputs: BTreeMap<String, BlockOutputData>,
    /// The block's operational state
    /// (`running | fault | disabled | terminated`).
    pub state: String,
    /// Fault reason when `state == "fault"`, else [`None`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault_reason: Option<String>,
}

impl From<BlockDefinition> for JsBlockInspection {
    fn from(block: BlockDefinition) -> Self {
        Self {
            id: block.id,
            name: block.name,
            lib: block.library,
            inputs: block.inputs,
            outputs: block.outputs,
            state: block.state,
            fault_reason: block.fault_reason,
        }
    }
}
