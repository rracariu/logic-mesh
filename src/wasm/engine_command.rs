// Copyright (c) 2022-2024, Radu Racariu.

//! Commands sent from JavaScript to the engine.

use std::fmt;

use js_sys::{Array, Function, Map, Object, Reflect};
use libhaystack::val::Value;
use serde::Serialize;
use uuid::Uuid;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

use crate::base::engine::EngineClient;
use crate::base::engine::messages::WatchId;
use crate::base::error::{EngineError, ErrorCode, RequestError, parse_block_uuid, parse_link_uuid};
use crate::base::program::{EditorMetadata, LinkData, Position, Program};
use crate::wasm::MAX_SAFE_INTEGER;
use crate::wasm::error::{js_error, js_error_from};
use crate::wasm::types::{JsBlockInspection, JsWatchNotification};

/// Commands a running instance of a Block Engine.
///
/// Every method sends one request and resolves with the engine's
/// response to it. Calls on one handle may overlap freely, and every
/// handle feeds the same engine, which handles requests in the order
/// they arrive.
///
/// A failed request rejects its promise with a `LogicMeshError`: an
/// `Error` whose `code` property is the stable name of the failure —
/// `BlockInstanceNotFound`, `UnsupportedProgramVersion`, … — and whose
/// `message` describes it.
#[wasm_bindgen]
pub struct EngineCommand {
    client: EngineClient,
}

/// Parses a block id from JavaScript.
fn block_id(uuid: &str) -> Result<Uuid, JsValue> {
    parse_block_uuid(uuid).map_err(|err| js_error(err.into()))
}

/// Converts a response for JavaScript.
fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|err| js_error_from(ErrorCode::Serialization, err.to_string()))
}

/// Converts a response for JavaScript as plain JSON-compatible values:
/// maps are objects, not `Map`s, and a missing value is `null`.
fn to_json_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| js_error_from(ErrorCode::Serialization, err.to_string()))
}

/// Reads a value written from JavaScript, for `target`. `null` and
/// `undefined` are `Null`; anything else must be a Haystack value in
/// its JSON encoding, or the request is rejected with the
/// `InvalidValue` code.
fn from_js_value(value: JsValue, target: impl fmt::Display) -> Result<Value, JsValue> {
    if value.is_null() || value.is_undefined() {
        return Ok(Value::Null);
    }
    let invalid = |detail: &dyn fmt::Display| {
        js_error_from(
            ErrorCode::InvalidValue,
            format!("Invalid value for {target}: {detail}"),
        )
    };
    if let Some(detail) = unencodable_object(&value, 0) {
        return Err(invalid(&detail));
    }
    serde_wasm_bindgen::from_value(value).map_err(|err| invalid(&err))
}

/// How deep a value written from JavaScript may nest. Haystack values
/// nest a few levels; the limit keeps a cyclic value from overflowing
/// the stack.
const MAX_VALUE_DEPTH: usize = 64;

/// Says why `value` cannot be a Haystack value in its JSON encoding if
/// it holds an object the encoding has no use for, or nests deeper than
/// [`MAX_VALUE_DEPTH`].
///
/// serde-wasm-bindgen reads any object that is not iterable through its
/// own enumerable properties, so it would read a `Date`, a class
/// instance or a boxed primitive as a Dict of whatever fields it
/// happens to have. Arrays, `Map`s and plain objects — whose prototype
/// is `null` or the `Object.prototype` of any realm — are looked into;
/// any other value is left for the deserializer to judge.
fn unencodable_object(value: &JsValue, depth: usize) -> Option<String> {
    if !value.is_object() {
        return None;
    }
    if depth == MAX_VALUE_DEPTH {
        return Some(format!(
            "it nests more than {MAX_VALUE_DEPTH} levels deep, or holds itself"
        ));
    }
    let nested = |item: JsValue| unencodable_object(&item, depth + 1);

    if Array::is_array(value) {
        return value.unchecked_ref::<Array>().iter().find_map(nested);
    }
    if let Some(map) = value.dyn_ref::<Map>() {
        let mut found = None;
        map.for_each(&mut |item, _key| {
            if found.is_none() {
                found = nested(item);
            }
        });
        return found;
    }

    let plain = match Reflect::get_prototype_of(value) {
        Ok(prototype) => {
            prototype.is_null()
                || Reflect::get_prototype_of(&prototype).is_ok_and(|parent| parent.is_null())
        }
        Err(_) => false,
    };
    if !plain {
        return Some(match constructor_name(value) {
            Some(name) => format!("{name} objects are not Haystack values"),
            None => "only plain objects and Maps are Haystack values".to_string(),
        });
    }
    Object::values(value.unchecked_ref::<Object>())
        .iter()
        .find_map(nested)
}

/// The name of the class `object` is an instance of, if it has one.
fn constructor_name(object: &JsValue) -> Option<String> {
    Reflect::get(object, &"constructor".into())
        .ok()?
        .dyn_into::<Function>()
        .ok()
        .map(|constructor| String::from(constructor.name()))
        .filter(|name| !name.is_empty())
}

/// Rejects a program whose `version` is newer than this crate reads
/// before its shape is read, so the rejection carries the
/// `UnsupportedProgramVersion` code rather than whatever field of the
/// newer format fails to deserialize.
fn check_program_version(program: &JsValue) -> Result<(), RequestError> {
    let version = js_sys::Reflect::get(program, &"version".into())
        .ok()
        .and_then(|version| version.as_f64());
    match version {
        Some(found)
            if found.fract() == 0.0
                && found > f64::from(Program::FORMAT_VERSION)
                && found <= f64::from(u32::MAX) =>
        {
            Err(EngineError::UnsupportedProgramVersion {
                found: found as u32,
                supported: Program::FORMAT_VERSION,
            }
            .into())
        }
        _ => Ok(()),
    }
}

#[wasm_bindgen]
impl EngineCommand {
    /// Creates a new engine command handle.
    pub(super) fn new(client: EngineClient) -> Self {
        Self { client }
    }

    /// Adds a block instance to the engine, immediately scheduling it
    /// for execution, and resolves with its id. Without `lib`, the block
    /// is looked up in every library, and a name more than one library
    /// holds is rejected.
    #[wasm_bindgen(js_name = "addBlock")]
    pub async fn add_block(
        &self,
        block_name: String,
        block_uuid: Option<String>,
        lib: Option<String>,
    ) -> Result<String, JsValue> {
        let block_id = block_uuid.as_deref().map(block_id).transpose()?;
        self.client
            .add_block(&block_name, block_id, lib.as_deref())
            .await
            .map(|id| id.to_string())
            .map_err(js_error)
    }

    /// Removes a block instance and all its links from the engine,
    /// returning the UUID of the removed block.
    #[wasm_bindgen(js_name = "removeBlock")]
    pub async fn remove_block(&self, block_uuid: String) -> Result<String, JsValue> {
        self.client
            .remove_block(block_id(&block_uuid)?)
            .await
            .map(|id| id.to_string())
            .map_err(js_error)
    }

    /// Links an output (or input) of one block to an input of another,
    /// and resolves with the link, its id assigned.
    #[wasm_bindgen(js_name = "createLink", unchecked_return_type = "LinkData")]
    pub async fn create_link(
        &self,
        source_block_uuid: String,
        target_block_uuid: String,
        source_block_pin_name: String,
        target_block_pin_name: String,
    ) -> Result<JsValue, JsValue> {
        let link = LinkData {
            id: None,
            source_block_uuid,
            target_block_uuid,
            source_block_pin_name,
            target_block_pin_name,
        };
        let link = self.client.connect(link).await.map_err(js_error)?;
        to_js(&link)
    }

    /// Removes a link by UUID, returning `true` if it was found and removed.
    #[wasm_bindgen(js_name = "removeLink")]
    pub async fn remove_link(&self, link_uuid: String) -> Result<bool, JsValue> {
        let link = parse_link_uuid(&link_uuid).map_err(|err| js_error(err.into()))?;
        self.client.disconnect(link).await.map_err(js_error)
    }

    /// Writes a value to a block's output pin, resolving with the value
    /// written.
    ///
    /// `value` is a Haystack value in its JSON encoding, made of plain
    /// objects, arrays and `Map`s; `null` and `undefined` write `Null`.
    /// Anything else, such as a `Date` or a function, rejects with the
    /// `InvalidValue` code.
    #[wasm_bindgen(js_name = "writeBlockOutput", unchecked_return_type = "unknown")]
    pub async fn write_block_output(
        &self,
        block_uuid: String,
        output_name: String,
        #[wasm_bindgen(unchecked_param_type = "unknown")] value: JsValue,
    ) -> Result<JsValue, JsValue> {
        let block = block_id(&block_uuid)?;
        let value = from_js_value(value, format_args!("output '{output_name}'"))?;
        let written = self
            .client
            .write_output(block, &output_name, value)
            .await
            .map_err(js_error)?;
        to_js(&written)
    }

    /// Writes a value to a block's input pin, resolving with the input's
    /// previous value, or `undefined` if it had none.
    ///
    /// `value` is a Haystack value in its JSON encoding, made of plain
    /// objects, arrays and `Map`s; `null` and `undefined` write `Null`.
    /// Anything else, such as a `Date` or a function, rejects with the
    /// `InvalidValue` code.
    #[wasm_bindgen(js_name = "writeBlockInput", unchecked_return_type = "unknown")]
    pub async fn write_block_input(
        &self,
        block_uuid: String,
        input_name: String,
        #[wasm_bindgen(unchecked_param_type = "unknown")] value: JsValue,
    ) -> Result<JsValue, JsValue> {
        let block = block_id(&block_uuid)?;
        let value = from_js_value(value, format_args!("input '{input_name}'"))?;
        let previous = self
            .client
            .write_input(block, &input_name, value)
            .await
            .map_err(js_error)?;
        to_js(&previous)
    }

    /// Loads a full program (blocks, links, pin values, UI and editor
    /// metadata) into the engine, all or nothing. The engine is
    /// expected to be empty: call `resetEngine` first if reloading.
    ///
    /// A program with no `version` is a save from before format
    /// versions, and loads as one. A program from a newer format is
    /// rejected with the `UnsupportedProgramVersion` code before its
    /// shape is read; one that is not a program at all with
    /// `InvalidProgram`.
    #[wasm_bindgen(js_name = "loadProgram")]
    pub async fn load_program(
        &self,
        #[wasm_bindgen(unchecked_param_type = "Program")] program: JsValue,
    ) -> Result<(), JsValue> {
        check_program_version(&program).map_err(js_error)?;
        let program: Program = serde_wasm_bindgen::from_value(program).map_err(|err| {
            js_error_from(
                ErrorCode::InvalidProgram,
                format!("Invalid program payload: {err}"),
            )
        })?;
        self.client.load_program(program).await.map_err(js_error)
    }

    /// Returns the engine's current program in the save format, as
    /// plain JSON-compatible objects: maps are objects, not `Map`s, and
    /// a missing value is `null`. Round-trips through `loadProgram`,
    /// editor metadata included.
    #[wasm_bindgen(js_name = "getProgram", unchecked_return_type = "Program")]
    pub async fn get_program(&self) -> Result<JsValue, JsValue> {
        let program = self.client.save_program().await.map_err(js_error)?;
        to_json_js(&program)
    }

    /// Resolves with a block's state and pin values, as plain
    /// JSON-compatible objects.
    #[wasm_bindgen(js_name = "inspectBlock", unchecked_return_type = "BlockInspection")]
    pub async fn inspect_block(&self, block_uuid: String) -> Result<JsValue, JsValue> {
        let block = self
            .client
            .inspect_block(block_id(&block_uuid)?)
            .await
            .map_err(js_error)?;
        to_json_js(&JsBlockInspection::from(block))
    }

    /// Evaluates a block by name, returning its output values.
    ///
    /// Creates a temporary block instance, feeds it the given `inputs`,
    /// executes it, and returns the outputs. Without `lib`, the block is
    /// looked up in every library, as for `addBlock`. Each input is read
    /// as `writeBlockInput` reads its value.
    #[wasm_bindgen(js_name = "evalBlock", unchecked_return_type = "unknown[]")]
    pub async fn eval_block(
        &self,
        block_name: String,
        #[wasm_bindgen(unchecked_param_type = "unknown[]")] inputs: Vec<JsValue>,
        lib: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let inputs = inputs
            .into_iter()
            .enumerate()
            .map(|(index, value)| from_js_value(value, format_args!("input {index}")))
            .collect::<Result<_, _>>()?;
        let outputs = self
            .client
            .eval_block(&block_name, inputs, lib.as_deref())
            .await
            .map_err(js_error)?;
        to_js(&outputs)
    }

    /// Updates the label and position a block is saved with: each of
    /// `label` and `position` that is given replaces the stored one.
    #[wasm_bindgen(js_name = "setBlockMetadata")]
    pub async fn set_block_metadata(
        &self,
        block_uuid: String,
        label: Option<String>,
        #[wasm_bindgen(unchecked_param_type = "Position | null | undefined")] position: JsValue,
    ) -> Result<(), JsValue> {
        let position: Option<Position> = serde_wasm_bindgen::from_value(position)
            .map_err(|err| js_error_from(ErrorCode::InvalidArgument, err.to_string()))?;
        self.client
            .set_block_metadata(block_id(&block_uuid)?, label, position)
            .await
            .map_err(js_error)
    }

    /// Replaces the editor metadata a block is saved with — any JSON
    /// object — or removes it if `editor` is `null` or `undefined`.
    #[wasm_bindgen(js_name = "setBlockEditor")]
    pub async fn set_block_editor(
        &self,
        block_uuid: String,
        #[wasm_bindgen(unchecked_param_type = "EditorMetadata | null | undefined")] editor: JsValue,
    ) -> Result<(), JsValue> {
        let editor: Option<EditorMetadata> = serde_wasm_bindgen::from_value(editor)
            .map_err(|err| js_error_from(ErrorCode::InvalidArgument, err.to_string()))?;
        self.client
            .set_block_editor(block_id(&block_uuid)?, editor)
            .await
            .map_err(js_error)
    }

    /// Opens a watch on block changes: from now on `callback` is called
    /// with a notification for every block change, until `removeWatch`
    /// closes the watch. Resolves with the watch's id once the engine
    /// opened it.
    ///
    /// Notifications are queued without bound between the engine and
    /// the callback, so a burst — such as many blocks faulting while a
    /// program loads — is never dropped.
    #[wasm_bindgen(js_name = "createWatch")]
    pub async fn create_watch(
        &self,
        #[wasm_bindgen(unchecked_param_type = "(notification: BlockNotification) => void")]
        callback: js_sys::Function,
    ) -> Result<f64, JsValue> {
        let watch = self.client.watch().await.map_err(js_error)?;
        let id = watch.id;
        let mut notifications = watch.notifications;
        // Ends once the engine drops the watch's sender: on
        // `removeWatch`, or with the engine.
        wasm_bindgen_futures::spawn_local(async move {
            while let Some(msg) = notifications.recv().await {
                let delivered = serde_wasm_bindgen::to_value::<JsWatchNotification>(&msg.into())
                    .map_err(|err| format!("Failed to serialize watch message: {err:?}"))
                    .and_then(|notification| {
                        callback
                            .call1(&JsValue::NULL, &notification)
                            .map_err(|err| format!("Failed to call watch callback: {err:?}"))
                    });
                if let Err(err) = delivered {
                    log::debug!(target: "create_watch", "{err}");
                }
            }
        });
        // Watch ids count up from 0, far below the range a JavaScript
        // number holds exactly.
        Ok(id.get() as f64)
    }

    /// Closes the watch `createWatch` resolved with `watch_id`, resolving
    /// with whether the engine had it open. Once this resolves, its
    /// callback is not called again.
    ///
    /// Rejects with the `InvalidArgument` code if `watch_id` cannot be
    /// a watch id.
    #[wasm_bindgen(js_name = "removeWatch")]
    pub async fn remove_watch(&self, watch_id: f64) -> Result<bool, JsValue> {
        if !(watch_id.fract() == 0.0 && (0.0..=MAX_SAFE_INTEGER).contains(&watch_id)) {
            return Err(js_error_from(
                ErrorCode::InvalidArgument,
                format!("Invalid watch id {watch_id}"),
            ));
        }
        self.client
            .unwatch(WatchId::new(watch_id as u64))
            .await
            .map_err(js_error)
    }

    /// Attaches an already-registered connector, by name, to the
    /// running engine: the engine resolves the name in the process-wide
    /// connector registry, starts the connector, and manages its
    /// lifecycle from then on. Register the connector first, with
    /// `registerConnector`.
    ///
    /// A paused engine still answers this request, like every other.
    #[wasm_bindgen(js_name = "addConnector")]
    pub async fn add_connector(&self, name: String) -> Result<String, JsValue> {
        self.client.add_connector(&name).await.map_err(js_error)
    }

    /// Detaches an engine-managed connector by name: the engine stops
    /// tracking it, unregisters it from the process-wide registry, and
    /// stops it. Returns the removed connector's name.
    ///
    /// A paused engine still answers this request, like every other.
    #[wasm_bindgen(js_name = "removeConnector")]
    pub async fn remove_connector(&self, name: String) -> Result<String, JsValue> {
        self.client.remove_connector(&name).await.map_err(js_error)
    }

    /// Lists the names of the engine-managed connectors.
    ///
    /// A paused engine still answers this request, like every other.
    #[wasm_bindgen(js_name = "listConnectors")]
    pub async fn list_connectors(&self) -> Result<Vec<String>, JsValue> {
        self.client.list_connectors().await.map_err(js_error)
    }

    /// Pauses block execution: no block executes until
    /// `resumeExecution`, while every other request is still answered.
    /// Connector IO tasks keep running. Does nothing if already paused.
    /// Resolves once the engine paused.
    #[wasm_bindgen(js_name = "pauseExecution")]
    pub async fn pause_execution(&self) -> Result<(), JsValue> {
        self.client.pause().await.map_err(js_error)
    }

    /// Resumes block execution. Does nothing if not paused. Resolves
    /// once the engine resumed.
    #[wasm_bindgen(js_name = "resumeExecution")]
    pub async fn resume_execution(&self) -> Result<(), JsValue> {
        self.client.resume().await.map_err(js_error)
    }

    /// Resets the engine state, clearing all blocks and links, and
    /// unregistering and stopping the engine-managed connectors — also
    /// while paused, and the engine stays paused. Resolves once the
    /// reset is done.
    #[wasm_bindgen(js_name = "resetEngine")]
    pub async fn reset_engine(&self) -> Result<(), JsValue> {
        self.client.reset().await.map_err(js_error)
    }

    /// Stops the engine: `run` resolves, keeping the blocks so a later
    /// `run` resumes them. Resolves once block execution and the
    /// managed connectors stopped.
    #[wasm_bindgen(js_name = "stopEngine")]
    pub async fn stop_engine(&self) -> Result<(), JsValue> {
        self.client.shutdown().await.map_err(js_error)
    }
}
