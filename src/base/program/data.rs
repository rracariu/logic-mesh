// Copyright (c) 2022-2026, Radu Racariu.

//! Serializable program format.
//!
//! [`Program`] is the canonical save/load shape — the engine can write it
//! out (`save_program`) and read it back (`load_program`) without going
//! through any external (JS) loader. This is what makes headless
//! deployments possible.
//!
//! Shape mirrors the JS-side [`Program`] interface so the wasm bridge is a
//! single serde round-trip, not a JS-side reassembly job.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

/// Backwards-compatible metadata wrapper. Predates [`Program`] and only
/// carries top-level descriptors; modern code uses [`Program::name`] /
/// [`Program::description`] directly. Kept because the
/// [`ProgramMeta`] type was part of the public surface.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProgramMeta {
    /// Program name.
    pub name: String,
    /// Libraries referenced by the program.
    pub libs: Vec<String>,
    /// Semantic version string.
    pub ver: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Program author.
    pub author: Option<String>,
    /// SPDX license identifier.
    pub license: Option<String>,
}

/// Link between two pins.
///
/// Fields use camelCase serialization to match the JS [`LinkData`]
/// interface — programs round-trip through the wasm bridge unchanged.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkData {
    /// Optional link UUID (omitted for auto-generated IDs).
    pub id: Option<String>,
    /// UUID of the source block.
    pub source_block_uuid: String,
    /// UUID of the target block.
    pub target_block_uuid: String,
    /// Output pin name on the source block.
    pub source_block_pin_name: String,
    /// Input pin name on the target block.
    pub target_block_pin_name: String,
}

/// Minimal block identity record used by inspect / snapshot APIs. Carries
/// the descriptor fields the engine knows about. For full
/// load/save round-trip use [`Program`] / [`ProgramBlock`] instead.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BlockData {
    /// Block UUID.
    pub id: String,
    /// Block type name.
    pub name: String,
    /// Display name.
    pub dis: String,
    /// Library the block belongs to.
    pub lib: String,
    /// Functional category.
    pub category: String,
    /// Semantic version.
    pub ver: String,
}

/// UI position metadata. Stored alongside the block on the engine side
/// so that headless save → reload round-trips the layout. The engine
/// itself never reads these — purely passthrough.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

/// Pin payload as stored in the program format. Carries the value plus
/// whether the pin is currently wired up. Both fields are independently
/// useful: connected inputs may still carry a last-known value, and
/// disconnected inputs (constants) have only a value.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinValue {
    /// The pin's current value.
    pub value: Value,
    /// Whether the pin is currently wired to another block.
    #[serde(default)]
    pub is_connected: bool,
}

/// One block entry in the program format. Mirrors the per-block object
/// in the JS [`Program`] interface.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProgramBlock {
    /// Block type name (looked up in the block registry at load time).
    pub name: String,
    /// Block library, e.g. `"core"`.
    pub lib: String,
    /// User-supplied display label shown alongside the block-type name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// UI position (engine-side passthrough only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<Position>,
    /// Input pins keyed by name. Includes constants (unconnected inputs
    /// with user-written values) as well as the last-known values of
    /// connected inputs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, PinValue>,
    /// Output pins keyed by name. Mostly informational — execute()
    /// recomputes outputs after load — but useful for visual continuity
    /// (UI shows last-known values before the engine catches up).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, PinValue>,
}

/// Full savable program: identity, all blocks keyed by uuid, all links
/// keyed by uuid. This is the format the engine's `save_program` emits
/// and `load_program` consumes. Round-trips through the wasm bridge as
/// JSON without reassembly.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::program::{Program, ProgramBlock};
///
/// let mut program = Program::default();
/// program.name = Some("my-program".to_string());
/// program.blocks.insert(
///     "00000000-0000-0000-0000-000000000000".to_string(),
///     ProgramBlock {
///         name: "Add".to_string(),
///         lib: "core".to_string(),
///         ..Default::default()
///     },
/// );
///
/// let json = serde_json::to_string(&program).unwrap();
/// let loaded: Program = serde_json::from_str(&json).unwrap();
/// assert_eq!(loaded.blocks.len(), 1);
/// ```
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Program {
    /// Program name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Blocks keyed by their UUID string.
    #[serde(default)]
    pub blocks: BTreeMap<String, ProgramBlock>,
    /// Links keyed by their UUID string.
    #[serde(default)]
    pub links: BTreeMap<String, LinkData>,
}
