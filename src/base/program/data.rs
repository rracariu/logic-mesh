// Copyright (c) 2022-2026, Radu Racariu.

//! Serializable program format.
//!
//! [`Program`] is the canonical save/load shape — the engine can write it
//! out (`save_program`) and read it back (`load_program`) without going
//! through any external (JS) loader. This is what makes headless
//! deployments possible.
//!
//! Shape mirrors the JS-side `Program` interface so the wasm bridge is a
//! single serde round-trip, not a JS-side reassembly job.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

/// Backwards-compatible metadata wrapper. Predates [`Program`] and only
/// carries top-level descriptors; modern code uses [`Program::name`] /
/// [`Program::description`] directly. Kept because the
/// `data::ProgramMeta` type was part of the public surface.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProgramMeta {
    pub name: String,
    pub libs: Vec<String>,
    pub ver: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
}

/// Link between two pins.
///
/// Fields use camelCase serialization to match the JS `LinkData`
/// interface — programs round-trip through the wasm bridge unchanged.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkData {
    pub id: Option<String>,
    pub source_block_uuid: String,
    pub target_block_uuid: String,
    pub source_block_pin_name: String,
    pub target_block_pin_name: String,
}

/// Minimal block identity record used by inspect / snapshot APIs. Carries
/// the descriptor fields the engine knows about. For full
/// load/save round-trip use [`Program`] / [`ProgramBlock`] instead.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BlockData {
    pub id: String,
    pub name: String,
    pub dis: String,
    pub lib: String,
    pub category: String,
    pub ver: String,
}

/// UI position metadata. Stored alongside the block on the engine side
/// so that headless save → reload round-trips the layout. The engine
/// itself never reads these — purely passthrough.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Pin payload as stored in the program format. Carries the value plus
/// whether the pin is currently wired up. Both fields are independently
/// useful: connected inputs may still carry a last-known value, and
/// disconnected inputs (constants) have only a value.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinValue {
    pub value: Value,
    #[serde(default)]
    pub is_connected: bool,
}

/// One block entry in the program format. Mirrors the per-block object
/// in the JS `Program` interface.
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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Program {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Blocks keyed by their UUID string.
    #[serde(default)]
    pub blocks: BTreeMap<String, ProgramBlock>,
    /// Links keyed by their UUID string.
    #[serde(default)]
    pub links: BTreeMap<String, LinkData>,
}
