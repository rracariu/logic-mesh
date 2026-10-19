// Copyright (c) 2022-2026, Radu Racariu.

//! Serializable program format.
//!
//! [`Program`] is the canonical save/load shape — the engine can write it
//! out (`save_program`) and read it back (`load_program`) without going
//! through any external (JS) loader. This is what makes headless
//! deployments possible.
//!
//! Shape mirrors the JS-side [`Program`] interface so the wasm bridge is a
//! single serde round-trip, not a JS-side reassembly job. The format
//! versions are described on [`crate::base::program`].

use std::collections::BTreeMap;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};

use crate::base::error::EngineError;

/// Opaque editor or UI state saved with a block: any JSON object.
///
/// The engine stores it without reading it and saves it back as it was
/// loaded, so an editor can keep its own state — which widget shows a
/// block, say — in the program. Through serde, and across the wasm
/// boundary to and from a plain JS object, nested objects, arrays,
/// strings, booleans, `null` and numbers come back as they went in; the
/// usual JSON limits apply: object keys come back sorted, and a JS
/// `undefined` comes back as `null`, as does any value JSON has no
/// encoding for.
pub type EditorMetadata = serde_json::Map<String, serde_json::Value>;

/// The version a save from before format versions reads as.
const fn legacy_format_version() -> u32 {
    0
}

/// Link between two pins.
///
/// Fields use camelCase serialization to match the JS [`LinkData`]
/// interface — programs round-trip through the wasm bridge unchanged.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::program::LinkData;
///
/// let link = LinkData::new(
///     "00000000-0000-0000-0000-000000000000",
///     "out",
///     "00000000-0000-0000-0000-000000000001",
///     "in0",
/// )
/// .with_id("00000000-0000-0000-0000-0000000000aa");
/// assert_eq!(link.target_block_pin_name, "in0");
/// ```
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct LinkData {
    /// Optional link UUID. An engine keeps the id a link is created or
    /// loaded with; left out, a connect request gets a fresh id, and a
    /// loaded link the key it has in [`Program::links`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

impl LinkData {
    /// A link from pin `source_pin` of block `source` to input
    /// `target_pin` of block `target`, with no id. The blocks are given
    /// by id — a [`Uuid`](crate::Uuid) or its string form.
    pub fn new(
        source: impl ToString,
        source_pin: impl Into<String>,
        target: impl ToString,
        target_pin: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            source_block_uuid: source.to_string(),
            target_block_uuid: target.to_string(),
            source_block_pin_name: source_pin.into(),
            target_block_pin_name: target_pin.into(),
        }
    }

    /// Names the link's id — a [`Uuid`](crate::Uuid) or its string form.
    pub fn with_id(mut self, id: impl ToString) -> Self {
        self.id = Some(id.to_string());
        self
    }
}

/// Minimal block identity record used by inspect / snapshot APIs. Carries
/// the descriptor fields the engine knows about. For full
/// load/save round-trip use [`Program`] / [`ProgramBlock`] instead.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct BlockData {
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
#[non_exhaustive]
pub struct Position {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
}

impl Position {
    /// The position at `x`, `y`.
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Pin payload as stored in the program format. Carries the value plus
/// whether the pin is currently wired up. Both fields are independently
/// useful: connected inputs may still carry a last-known value, and
/// disconnected inputs (constants) have only a value.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PinValue {
    /// The pin's current value. Required when read; a pin with no value
    /// saves [`Value::Null`], which a load does not write.
    pub value: Value,
    /// Whether the pin is currently wired to another block. `false` when
    /// left out.
    #[serde(default)]
    pub is_connected: bool,
}

impl PinValue {
    /// A pin holding `value`, wired to another block if `is_connected`.
    pub fn new(value: impl Into<Value>, is_connected: bool) -> Self {
        Self {
            value: value.into(),
            is_connected,
        }
    }
}

/// One block entry in the program format. Mirrors the per-block object
/// in the JS [`Program`] interface.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::program::{PinValue, Position, ProgramBlock};
///
/// let mut block = ProgramBlock::new("Add", "core");
/// block.label = Some("Demand sum".to_string());
/// block.position = Some(Position::new(120.0, 80.0));
/// block.inputs.insert("in0".to_string(), PinValue::new(5, false));
/// ```
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProgramBlock {
    /// Block type name (looked up in the block registry at load time).
    pub name: String,
    /// Block library, e.g. `"core"`.
    pub lib: String,
    /// The version of the block type the program was saved with, its
    /// [`BlockDesc::ver`](crate::base::block::BlockDesc::ver). A load
    /// rejects a block whose saved version is incompatible with the
    /// registered one — a different major version, or a different
    /// minor version below 1.0 — with
    /// [`EngineError::BlockVersionMismatch`], and logs a warning for
    /// any other difference. Left out, the block is not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ver: Option<String>,
    /// User-supplied display label shown alongside the block-type name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// UI position (engine-side passthrough only). Read from `positions`
    /// too, the key a save from before format versions used.
    #[serde(default, alias = "positions", skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// Opaque editor metadata (engine-side passthrough only), saved back
    /// exactly as it was loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<EditorMetadata>,
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

impl ProgramBlock {
    /// A block of type `name` from library `lib`, with no metadata and
    /// no pin values.
    pub fn new(name: impl Into<String>, lib: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            lib: lib.into(),
            ..Default::default()
        }
    }
}

/// Full savable program: identity, all blocks keyed by uuid, all links
/// keyed by uuid. This is the format the engine's `save_program` emits
/// and `load_program` consumes. Round-trips through the wasm bridge as
/// JSON without reassembly.
///
/// See the [module docs](crate::base::program#format-versions) for the
/// format versions a program is read in.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::program::{Program, ProgramBlock};
///
/// let mut program = Program::new();
/// program.name = Some("my-program".to_string());
/// program.blocks.insert(
///     "00000000-0000-0000-0000-000000000000".to_string(),
///     ProgramBlock::new("Add", "core"),
/// );
///
/// let json = serde_json::to_string(&program).unwrap();
/// let loaded: Program = serde_json::from_str(&json).unwrap();
/// assert_eq!(loaded, program);
/// assert_eq!(loaded.version, Program::FORMAT_VERSION);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Program {
    /// The format version the program is in. A new program is in
    /// [`FORMAT_VERSION`](Self::FORMAT_VERSION); a save from before
    /// format versions, which has none, reads as 0.
    #[serde(default = "legacy_format_version")]
    pub version: u32,
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

impl Program {
    /// The format version this crate writes, and the newest it reads.
    pub const FORMAT_VERSION: u32 = 1;

    /// An empty program in the current format.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks that this crate reads the format the program is in: one
    /// no newer than [`FORMAT_VERSION`](Self::FORMAT_VERSION).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::UnsupportedProgramVersion`] for a program
    /// from a newer format.
    pub fn check_version(&self) -> Result<(), EngineError> {
        check_format_version(self.version)
    }
}

impl Default for Program {
    fn default() -> Self {
        Self {
            version: Self::FORMAT_VERSION,
            name: None,
            description: None,
            blocks: BTreeMap::new(),
            links: BTreeMap::new(),
        }
    }
}

/// A program as the logic-mesh 1.0 web editor saved it: no format
/// version, `positions`, outputs without `isConnected`, and links keyed
/// by id without naming it.
#[cfg(test)]
pub(crate) const PROGRAM_1_0_JSON: &str = include_str!("testdata/program-1.0.json");

/// [`Program::check_version`] for a version read on its own.
pub(crate) fn check_format_version(found: u32) -> Result<(), EngineError> {
    if found > Program::FORMAT_VERSION {
        Err(EngineError::UnsupportedProgramVersion {
            found,
            supported: Program::FORMAT_VERSION,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;
    use serde_json::json;

    use super::*;

    #[test]
    fn a_1_0_program_reads_as_the_legacy_version() {
        let program: Program = serde_json::from_str(PROGRAM_1_0_JSON).expect("a 1.0 save parses");

        assert_eq!(program.version, 0);
        assert_matches!(program.check_version(), Ok(()));
        assert_eq!(program.name.as_deref(), Some("Demand sum"));
        let first = &program.blocks["00000000-0000-4000-8000-000000000001"];
        assert_eq!((first.name.as_str(), first.lib.as_str()), ("Add", "core"));
        assert_eq!(first.label.as_deref(), Some("Zone A + B"));
        assert_eq!(first.position, Some(Position::new(81.0, -72.5)));
        assert_eq!(first.inputs["in0"], PinValue::new(12, false));
        assert_eq!(first.outputs["out"], PinValue::new(19, false));
        assert_eq!((&first.ver, &first.editor), (&None, &None));
        let link = &program.links["00000000-0000-4000-8000-0000000000aa"];
        assert_eq!(link.id, None);
        assert_eq!(link.target_block_pin_name, "in0");
    }

    /// The JSON keys of every field, and that a program reads back as
    /// it was written.
    #[test]
    fn a_current_program_round_trips() {
        let mut editor = EditorMetadata::new();
        editor.insert("widget".to_string(), json!({"kind": "Slider"}));
        let mut block = ProgramBlock::new("Add", "core");
        block.ver = Some("1.0.0".to_string());
        block.label = Some("sum".to_string());
        block.position = Some(Position::new(1.5, -2.0));
        block.editor = Some(editor);
        block
            .inputs
            .insert("in0".to_string(), PinValue::new(3, true));
        let mut program = Program::new();
        program.name = Some("p".to_string());
        program.description = Some("d".to_string());
        program.blocks.insert("b".to_string(), block);
        program
            .links
            .insert("l".to_string(), LinkData::new("b", "out", "b", "in1"));

        let written = serde_json::to_value(&program).expect("serializes");
        assert_eq!(
            written,
            json!({
                "version": Program::FORMAT_VERSION,
                "name": "p",
                "description": "d",
                "blocks": {"b": {
                    "name": "Add",
                    "lib": "core",
                    "ver": "1.0.0",
                    "label": "sum",
                    "position": {"x": 1.5, "y": -2.0},
                    "editor": {"widget": {"kind": "Slider"}},
                    "inputs": {"in0": {"value": 3, "isConnected": true}},
                }},
                "links": {"l": {
                    "sourceBlockUuid": "b",
                    "targetBlockUuid": "b",
                    "sourceBlockPinName": "out",
                    "targetBlockPinName": "in1",
                }},
            })
        );
        let read: Program = serde_json::from_value(written).expect("reads back");
        assert_eq!(read, program);
    }

    #[test]
    fn a_program_from_a_newer_format_is_rejected() {
        let program: Program = serde_json::from_value(json!({
            "version": Program::FORMAT_VERSION + 1,
            "blocks": {},
        }))
        .expect("parses");

        assert_matches!(
            program.check_version(),
            Err(EngineError::UnsupportedProgramVersion { found, supported })
                if found == Program::FORMAT_VERSION + 1 && supported == Program::FORMAT_VERSION
        );
    }

    /// Editor metadata comes back exactly as it was written: nested
    /// objects and arrays, integers and fractions, nulls, booleans,
    /// strings and empty containers.
    #[test]
    fn editor_metadata_round_trips_exactly() {
        let editor = json!({
            "widget": {
                "kind": "MultiChart",
                "config": {
                    "series": [{"label": "OAT", "address": null}, {"label": ""}],
                    "min": -40,
                    "max": 9007199254740991_u64,
                    "step": 0.1,
                    "ratio": 1e-7,
                    "visible": true,
                    "empty": {},
                    "none": [],
                },
            },
            "_kind": "marker",
        });
        let written = json!({"version": 1, "blocks": {"b": {
            "name": "Add",
            "lib": "core",
            "editor": editor,
        }}, "links": {}});

        let program: Program = serde_json::from_str(&written.to_string()).expect("parses");
        assert_eq!(
            program.blocks["b"].editor.as_ref().map(|e| json!(e)),
            Some(editor)
        );
        assert_eq!(serde_json::to_value(&program).expect("serializes"), written);
    }

    /// A save naming the position under both keys is malformed.
    #[test]
    fn position_and_positions_together_are_rejected() {
        let block = json!({
            "name": "Add",
            "lib": "core",
            "position": {"x": 1.0, "y": 2.0},
            "positions": {"x": 1.0, "y": 2.0},
        });
        assert!(serde_json::from_value::<ProgramBlock>(block).is_err());
    }
}
