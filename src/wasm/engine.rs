// Copyright (c) 2022-2023, Radu Racariu.

//! Block engine exposed to JavaScript.

use crate::blocks::registry::{list_registered_blocks, register_block_desc};
use crate::single_threaded::SingleThreadedEngine;
use crate::wasm::engine_command::EngineCommand;
use crate::wasm::error::{js_error, js_error_from, js_type_name};
use crate::wasm::js_block::JS_FNS;
use crate::wasm::types::{JsBlockDesc, to_js_value};
use js_sys::Array;

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

use crate::base::engine::Engine;
use crate::base::error::{BlockDescError, ErrorCode, RequestError};

/// Controls the execution of blocks.
///
/// Loads programs and enables inspection and debugging
/// of blocks and their inputs and outputs.
#[wasm_bindgen]
pub struct BlocksEngine {
    engine: SingleThreadedEngine,
}

#[wasm_bindgen]
impl BlocksEngine {
    /// Creates a new engine instance.
    ///
    /// `scan_interval` is the engine's scan interval in milliseconds:
    /// how long a block that runs on every cycle waits between runs.
    /// Left out, the engine's default applies. A fraction of a
    /// millisecond is dropped.
    ///
    /// Throws a `LogicMeshError` with the `InvalidArgument` code if
    /// `scan_interval` is negative, not finite, or above
    /// `Number.MAX_SAFE_INTEGER`.
    #[wasm_bindgen(constructor)]
    pub fn new(scan_interval: Option<f64>) -> Result<BlocksEngine, JsValue> {
        let mut engine = SingleThreadedEngine::default();
        if let Some(scan_interval) = scan_interval {
            engine.set_scan_interval(crate::wasm::scan_interval(scan_interval)?);
        }
        Ok(Self { engine })
    }

    /// Lists the descriptions of all registered blocks.
    #[wasm_bindgen(js_name = "listBlocks", unchecked_return_type = "BlockDesc[]")]
    pub fn list_blocks(&self) -> Array {
        let arr = Array::new();

        list_registered_blocks().into_iter().for_each(|block| {
            if let Ok(desc) = to_js_value(&JsBlockDesc::from(block)) {
                arr.push(&desc);
            }
        });

        arr
    }

    /// Registers a new JS block in the registry and returns its name.
    ///
    /// `func`, if provided, is a factory that returns the function
    /// executing the block — without it (`null` or `undefined`) the
    /// block is a no-op. Pin kinds must be the lowercase Haystack names
    /// (`"number"`, `"str"`, …).
    ///
    /// Throws a `LogicMeshError`: `InvalidArgument` if `desc` is not a
    /// block description or `func` is neither a function nor left out,
    /// `UnknownPinKind`, `UnknownRunCondition` or `BlockNotExternal` for
    /// a bad field, and `BlockAlreadyRegistered` if the library already
    /// has a block of that name. Nothing is registered then.
    #[wasm_bindgen(js_name = "registerBlock")]
    pub fn register_block(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "BlockDesc")] desc: JsValue,
        // A custom type loses the `?` of an optional parameter; a
        // trailing parameter that takes `void` may be left out too.
        #[wasm_bindgen(unchecked_param_type = "JsBlockFactory | null | void")] func: JsValue,
    ) -> Result<String, JsValue> {
        let desc: JsBlockDesc = serde_wasm_bindgen::from_value(desc).map_err(|err| {
            js_error_from(
                ErrorCode::InvalidArgument,
                format!("Invalid block description: {err}"),
            )
        })?;

        let name = desc.name.clone();
        let lib = desc.lib.clone();

        // A bad field carries what is wrong with it as its source, and
        // the request error's message renders the chain to name both.
        let desc: crate::base::block::BlockDesc = desc
            .try_into()
            .map_err(|err: BlockDescError| js_error(RequestError::from(err)))?;
        // wasm-bindgen checks no argument against its declared type, so
        // read the factory before registering anything: a bad one would
        // only show when the block is added, with its name already taken.
        let factory = block_factory(func)?;
        register_block_desc(&desc).map_err(|err| js_error(RequestError::from(err)))?;

        // Registered with or without a factory, so a block without one
        // resolves to a no-op whatever else its library holds.
        JS_FNS.with_borrow_mut(|reg| {
            reg.entry(lib).or_default().insert(name.clone(), factory);
        });

        Ok(name)
    }

    /// Registers a JavaScript-implemented connector under `name` in
    /// the process-wide connector registry, as the module-level
    /// `registerConnector` function does, and throws as it does.
    ///
    /// Attach the registered connector to the running engine with the
    /// `addConnector` engine command.
    #[wasm_bindgen(js_name = "registerConnector")]
    pub fn register_connector(
        &self,
        name: String,
        #[wasm_bindgen(unchecked_param_type = "JsConnector")] connector: JsValue,
    ) -> Result<(), JsValue> {
        crate::wasm::js_connector::register_js_connector(name, connector)
    }

    /// Removes the connector registered under `name` from the
    /// process-wide connector registry, returning whether a connector
    /// was removed, as the module-level `unregisterConnector` function
    /// does.
    ///
    /// Meant for a connector that was registered but never attached. A
    /// connector attached to the running engine should instead be
    /// removed with the `removeConnector` engine command, which
    /// detaches and stops it before unregistering.
    #[wasm_bindgen(js_name = "unregisterConnector")]
    pub fn unregister_connector(&self, name: String) -> bool {
        crate::wasm::js_connector::unregister_js_connector(name)
    }

    /// Returns a new `EngineCommand` handle for sending commands.
    #[wasm_bindgen(js_name = "engineCommand")]
    pub fn engine_command(&mut self) -> EngineCommand {
        EngineCommand::new(self.engine.client())
    }

    /// Runs the engine asynchronously.
    ///
    /// After this is called, the engine instance can't be used directly —
    /// use the command object to communicate with the engine instead.
    #[wasm_bindgen]
    pub async fn run(&mut self) {
        // `run` consumes the idle engine and hands it back on shutdown;
        // an empty engine holds its place meanwhile, unreachable since
        // this call keeps `self` borrowed until the engine stops.
        let engine = std::mem::take(&mut self.engine);
        self.engine = engine.run().await;
    }
}

/// Reads the block factory `registerBlock` was given: `None` for `null`
/// or `undefined`.
fn block_factory(func: JsValue) -> Result<Option<js_sys::Function>, JsValue> {
    if func.is_null() || func.is_undefined() {
        return Ok(None);
    }
    func.dyn_into::<js_sys::Function>()
        .map(Some)
        .map_err(|func| {
            js_error_from(
                ErrorCode::InvalidArgument,
                format!(
                    "Invalid block factory: expected a function, null or undefined, got {}",
                    js_type_name(&func)
                ),
            )
        })
}
