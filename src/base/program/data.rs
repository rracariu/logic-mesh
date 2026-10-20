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
use std::fmt;

use libhaystack::val::Value;
use serde::de::{self, DeserializeOwned, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};

use crate::base::error::EngineError;

/// Opaque editor or UI state saved with a block: any JSON object.
///
/// The engine stores it without reading it and saves it back as it was
/// loaded or [set](crate::base::engine::Engine::set_block_editor), so
/// an editor can keep its own state — which widget shows a block, say —
/// in the program. Through serde, and across the wasm
/// boundary to and from a plain JS object, nested objects, arrays,
/// strings, booleans, `null` and numbers come back as they went in; the
/// usual JSON limits apply: object keys come back sorted, and a JS
/// `undefined` comes back as `null`, as does any value JSON has no
/// encoding for.
pub type EditorMetadata = serde_json::Map<String, serde_json::Value>;

/// The version a save from before format versions reads as.
const LEGACY_FORMAT_VERSION: u32 = 0;

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
    /// Block library, e.g. `"core"`. Saved under the key `lib`, the
    /// name the JS `Program` interface and earlier saves use.
    #[serde(rename = "lib")]
    pub library: String,
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
    /// A block of type `name` from `library`, with no metadata and no
    /// pin values.
    pub fn new(name: impl Into<String>, library: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            library: library.into(),
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
/// # Reading a save
///
/// Deserializing a program reads its `version` before its blocks and
/// links, and fails on a program from a newer format with the message
/// of [`EngineError::UnsupportedProgramVersion`], whatever the rest of
/// its shape: a newer format may have changed it so that it no longer
/// parses as this one. An engine's load checks a program built in code
/// the same way, and reports the typed error.
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
// Deserialized by hand, by `ProgramVisitor`, so that the version is
// checked first. Its JSON keys are the field names serialized here.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Program {
    /// The format version the program is in. A new program is in
    /// [`FORMAT_VERSION`](Self::FORMAT_VERSION); a save from before
    /// format versions, which has none, reads as 0.
    pub version: u32,
    /// Program name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Blocks keyed by their UUID string. Missing reads as empty.
    pub blocks: BTreeMap<String, ProgramBlock>,
    /// Links keyed by their UUID string. Missing reads as empty.
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
fn check_format_version(found: u32) -> Result<(), EngineError> {
    if found > Program::FORMAT_VERSION {
        Err(EngineError::UnsupportedProgramVersion {
            found,
            supported: Program::FORMAT_VERSION,
        })
    } else {
        Ok(())
    }
}

/// The keys of a [`Program`], `version` first: a deserializer that
/// visits a struct's fields by name, as serde-wasm-bindgen does, reads
/// them in this order.
const PROGRAM_FIELDS: &[&str] = &["version", "name", "description", "blocks", "links"];

/// A key of a [`Program`]. Any other key is ignored, as a derived
/// `Deserialize` would.
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum ProgramField {
    Version,
    Name,
    Description,
    Blocks,
    Links,
    #[serde(other)]
    Other,
}

impl<'de> Deserialize<'de> for Program {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_struct("Program", PROGRAM_FIELDS, ProgramVisitor)
    }
}

/// Reads a [`Program`], checking its version before its blocks and
/// links are read.
///
/// The keys of a JSON object can come in any order. Blocks or links
/// that come before the version are held as JSON, and read only once
/// the version is known: the whole program has been seen, or the
/// version came and passed the check.
struct ProgramVisitor;

impl<'de> Visitor<'de> for ProgramVisitor {
    type Value = Program;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a logic-mesh program")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Program, A::Error> {
        let mut version = None;
        let mut name = None;
        let mut description = None;
        let mut blocks = None;
        let mut links = None;
        while let Some(field) = map.next_key()? {
            match field {
                ProgramField::Version => {
                    if version.is_some() {
                        return Err(de::Error::duplicate_field("version"));
                    }
                    let found = map.next_value()?;
                    check_format_version(found).map_err(de::Error::custom)?;
                    version = Some(found);
                }
                ProgramField::Name => {
                    if name.is_some() {
                        return Err(de::Error::duplicate_field("name"));
                    }
                    name = Some(map.next_value::<Option<String>>()?);
                }
                ProgramField::Description => {
                    if description.is_some() {
                        return Err(de::Error::duplicate_field("description"));
                    }
                    description = Some(map.next_value::<Option<String>>()?);
                }
                ProgramField::Blocks => {
                    if blocks.is_some() {
                        return Err(de::Error::duplicate_field("blocks"));
                    }
                    blocks = Some(Entries::next(&mut map, version.is_some())?);
                }
                ProgramField::Links => {
                    if links.is_some() {
                        return Err(de::Error::duplicate_field("links"));
                    }
                    links = Some(Entries::next(&mut map, version.is_some())?);
                }
                ProgramField::Other => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
        }

        Ok(Program {
            version: version.unwrap_or(LEGACY_FORMAT_VERSION),
            name: name.flatten(),
            description: description.flatten(),
            blocks: Entries::finish(blocks)?,
            links: Entries::finish(links)?,
        })
    }
}

/// A program's blocks or links, as [`ProgramVisitor`] read them.
enum Entries<T> {
    /// Read after the version was checked.
    Read(BTreeMap<String, T>),
    /// Held as JSON, to be read once the version is known.
    Held(serde_json::Value),
}

impl<T: DeserializeOwned> Entries<T> {
    /// Reads the map's next value: as entries once the program's
    /// version is `checked`, as JSON otherwise.
    fn next<'de, A: MapAccess<'de>>(map: &mut A, checked: bool) -> Result<Self, A::Error> {
        if checked {
            map.next_value().map(Self::Read)
        } else {
            map.next_value().map(Self::Held)
        }
    }

    /// The entries read, empty if the program had none.
    fn finish<E: de::Error>(entries: Option<Self>) -> Result<BTreeMap<String, T>, E> {
        match entries {
            None => Ok(BTreeMap::new()),
            Some(Self::Read(entries)) => Ok(entries),
            Some(Self::Held(json)) => serde_json::from_value(json).map_err(E::custom),
        }
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
        assert_eq!(
            (first.name.as_str(), first.library.as_str()),
            ("Add", "core")
        );
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
    fn a_program_from_a_newer_format_fails_the_version_check() {
        let mut program = Program::new();
        program.version = Program::FORMAT_VERSION + 1;

        assert_matches!(
            program.check_version(),
            Err(EngineError::UnsupportedProgramVersion { found, supported })
                if found == Program::FORMAT_VERSION + 1 && supported == Program::FORMAT_VERSION
        );
    }

    /// Regression: a program from a newer format whose blocks no longer
    /// parse as this format's failed to deserialize with an error about
    /// a block field (``missing field `name` ``) instead of its version,
    /// so a caller reading a save could not tell why.
    #[test]
    fn a_newer_format_is_rejected_before_its_shape_is_read() {
        let newer = Program::FORMAT_VERSION + 1;
        let expected = EngineError::UnsupportedProgramVersion {
            found: newer,
            supported: Program::FORMAT_VERSION,
        }
        .to_string();
        let blocks = r#"{"b": {"type": "Add", "pins": [1, 2]}}"#;
        let links = r#"{"l": {"from": "b.out", "to": "b.in1"}}"#;

        for json in [
            format!(r#"{{"version": {newer}, "blocks": {blocks}, "links": {links}}}"#),
            format!(r#"{{"blocks": {blocks}, "links": {links}, "version": {newer}}}"#),
            format!(r#"{{"blocks": {blocks}, "version": {newer}, "links": {links}}}"#),
        ] {
            let err = serde_json::from_str::<Program>(&json).expect_err("a newer format");
            assert!(
                err.to_string().starts_with(&expected),
                "{json}: got `{err}`"
            );
        }
    }

    /// A program reads the same whether its version comes before or
    /// after its blocks and links, and with no version at all.
    #[test]
    fn a_program_reads_the_same_whatever_its_key_order() {
        let mut editor = EditorMetadata::new();
        editor.insert("widget".to_string(), json!({"kind": "Slider", "step": 0.1}));
        let mut block = ProgramBlock::new("Add", "core");
        block.ver = Some("1.0.0".to_string());
        block.position = Some(Position::new(1.0, 2.0));
        block.editor = Some(editor);
        block
            .inputs
            .insert("in0".to_string(), PinValue::new(2.5, false));
        let mut program = Program::new();
        program.name = Some("p".to_string());
        program.blocks.insert("b".to_string(), block);
        program
            .links
            .insert("l".to_string(), LinkData::new("b", "out", "b", "in1"));
        let blocks = serde_json::to_string(&program.blocks).expect("serializes");
        let links = serde_json::to_string(&program.links).expect("serializes");

        for json in [
            format!(r#"{{"version": 1, "name": "p", "blocks": {blocks}, "links": {links}}}"#),
            format!(r#"{{"name": "p", "blocks": {blocks}, "links": {links}, "version": 1}}"#),
            format!(r#"{{"links": {links}, "version": 1, "blocks": {blocks}, "name": "p"}}"#),
        ] {
            let read: Program = serde_json::from_str(&json).expect("reads");
            assert_eq!(read, program, "{json}");
        }

        let legacy = format!(r#"{{"name": "p", "blocks": {blocks}, "links": {links}}}"#);
        let read: Program = serde_json::from_str(&legacy).expect("reads");
        assert_eq!(read.version, 0);
        assert_eq!((read.blocks, read.links), (program.blocks, program.links));
    }

    /// The keys a program reads as a derived `Deserialize` would: a
    /// repeated key is an error, an unknown one is ignored, a null name
    /// or description is none, and missing blocks or links are empty.
    #[test]
    fn program_keys_read_as_a_derived_deserialize_would() {
        for key in PROGRAM_FIELDS {
            let value = if *key == "version" { "1" } else { "null" };
            let json = format!(r#"{{"{key}": {value}, "{key}": {value}}}"#);
            let err = serde_json::from_str::<Program>(&json).expect_err("a repeated key");
            assert!(
                err.to_string()
                    .contains(&format!("duplicate field `{key}`")),
                "{json}: got `{err}`"
            );
        }

        let read: Program =
            serde_json::from_str(r#"{"version": 1, "name": null, "description": null, "x": [1]}"#)
                .expect("reads");
        assert_eq!(read, Program::new());

        assert!(serde_json::from_str::<Program>(r#"{"version": 1, "blocks": null}"#).is_err());
        assert!(serde_json::from_str::<Program>(r#"{"version": -1}"#).is_err());
        assert!(serde_json::from_str::<Program>("[]").is_err());
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

    /// The Rust field is `library`, like every other type that names a
    /// block's library, but the save format keeps the key `lib`.
    #[test]
    fn the_library_is_saved_under_lib() {
        let block = ProgramBlock {
            library: "custom".to_string(),
            ..ProgramBlock::new("Scale", "core")
        };

        let written = serde_json::to_value(&block).expect("serializes");
        assert_eq!(written, json!({"name": "Scale", "lib": "custom"}));

        let read: ProgramBlock =
            serde_json::from_value(json!({"name": "Scale", "lib": "custom"})).expect("reads");
        assert_eq!(read.library, "custom");
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
