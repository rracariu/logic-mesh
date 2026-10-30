// Copyright (c) 2022-2023, Radu Racariu.

//! JavaScript-backed block implementation.
//!
//! Crate-private: JavaScript registers these blocks with
//! `registerBlock`, so nothing here is Rust API.

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use crate::base::error::{ExternalError, Result};
use js_sys::Promise;
use libhaystack::val::Value;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::base::block::BlockRunCondition;
use crate::base::block::{Block, BlockState};

use crate::blocks::registry::{BlockSink, eval_block_impl};
use crate::tokio_impl::block::drain_ready_inputs;
use crate::tokio_impl::scan::scan_interval_millis;
use crate::wasm::error::{js_detail, js_type_name};
use crate::wasm::types::to_js_value;
use crate::{
    base::{
        block::{BlockDesc, BlockProps},
        input::{InputProps, input_reader::InputReader},
        link::Link,
        output::{Output, OutputProps},
    },
    blocks::{InputImpl, OutputImpl},
};

/// The factory of every block `registerBlock` registered, by library
/// and block name — `None` for a block registered without one.
type ExternFuncRegistryType = HashMap<String, HashMap<String, Option<js_sys::Function>>>;

thread_local! {
    /// A registry of JS functions that implement block logic.
    pub(crate) static JS_FNS: RefCell<ExternFuncRegistryType> = RefCell::new(HashMap::new());
}

/// A block that is implemented in JavaScript.
///
/// The block delegates evaluation to a JS function that is called with the
/// inputs as arguments.
///
/// # Delivery semantics
///
/// The block actor drops an in-flight `execute` whenever an engine
/// command for the block arrives, and unlike the standard-library
/// blocks a JsBlock has a second await *after* the input drain — the JS
/// function's Promise. A cancellation landing there cannot un-run the
/// JS function (only the Promise's eventual result is discarded), and
/// the drained reaction is held on the block and the function
/// re-invoked with the current input values on the next cycle. A JS
/// call may therefore run more than once for one reaction, so
/// side-effecting block functions should be idempotent (at-least-once
/// semantics). Outputs are only ever set by a call that ran to
/// completion.
///
/// Unlike the external blocks, no timeout races the Promise await: a
/// call that never settles parks the reaction until the next engine
/// command for the block, and each such command re-invokes the
/// function afresh while every earlier never-settling Promise stays
/// alive — under sustained command traffic arriving faster than the
/// executor completes, the function re-fires on every command and the
/// outputs are never committed. Keep executors bounded in time as
/// well as idempotent.
#[derive(Debug, Default)]
pub(crate) struct JsBlock {
    id: Uuid,
    desc: BlockDesc,
    inputs: Vec<InputImpl>,
    outputs: Vec<OutputImpl>,
    state: BlockState,
    func: Option<js_sys::Function>,
    /// Whether an input drain has been consumed without the JS call it
    /// triggered running to completion: set once the input wait
    /// returns, kept across a cancelled `execute` so the call is
    /// re-issued, and cleared on any completion — outputs set, JS
    /// error, or serialization failure. Unlike `ExternalOut::pending`
    /// this holds no value snapshot: the drain already moved the
    /// values into the input caches, which persist on the block, and
    /// the retry re-reads them — so a value written between the
    /// cancellation and the retry (typically by the very `WriteInput`
    /// whose arrival cancelled us, which lands in the cache at once)
    /// supersedes the stale reaction, the same freshest-wins choice as
    /// ExternalOut's "a fresh value replaces any held retry".
    pending: bool,
}

impl JsBlock {
    /// Creates a new instance of a block.
    pub(crate) fn new(
        desc: BlockDesc,
        func: Option<js_sys::Function>,
        block_id: Option<Uuid>,
    ) -> Self {
        let id = block_id.unwrap_or_else(uuid::Uuid::new_v4);

        let inputs = desc
            .inputs
            .iter()
            .map(|input| InputImpl::new(&input.name, input.kind, id))
            .collect::<Vec<_>>();

        let outputs = desc
            .outputs
            .iter()
            .map(|input| OutputImpl::new_named(&input.name, input.kind, id))
            .collect::<Vec<_>>();

        Self {
            id,
            desc,
            inputs,
            outputs,
            state: BlockState::Running,
            func,
            pending: false,
        }
    }

    /// Calls the JS function with the inputs as arguments — in the
    /// bindings' one encoding, so an input with no value is `null` —
    /// and sets the outputs from the array it returns or resolves with.
    /// Anything else leaves the outputs as they are. A throw or a
    /// rejection faults the block, with the thrown value as the reason.
    ///
    /// A fault is logged at debug only: it reaches JavaScript through
    /// the block's state and fault reason, and a block that runs on
    /// every cycle would log it on every cycle.
    async fn call_js_function(&mut self, values: JsValue) {
        let Some(ref func) = self.func else {
            return;
        };

        // `Promise::resolve` takes a plain return value, a native
        // Promise and any thenable alike: a synchronous executor's
        // array is not a Promise, and awaiting it as one would call its
        // missing `then`.
        let settled = match func.call1(&JsValue::NULL, &values) {
            Ok(result) => JsFuture::from(Promise::resolve(&result)).await,
            Err(err) => Err(err),
        };

        let result = match settled {
            Ok(result) => result,
            Err(err) => {
                let detail = js_detail(&err);
                log::debug!("JS function of block '{}' failed: {detail}", self.desc.name);
                self.set_state(BlockState::fault(format!("JS execute failed: {detail}")));
                return;
            }
        };

        if !result.is_array() {
            return;
        }

        match serde_wasm_bindgen::from_value::<Vec<Value>>(result) {
            Ok(list) => list.into_iter().enumerate().for_each(|(index, value)| {
                if let Some(output) = self.outputs.get_mut(index) {
                    output.set(value);
                }
            }),
            Err(err) => {
                log::debug!("Failed to process the return of the JS function: {err}");
                self.set_state(BlockState::fault(format!(
                    "JS return processing failed: {err}"
                )));
            }
        }
    }
}

impl BlockProps for JsBlock {
    type Reader = <InputImpl as InputProps>::Reader;
    type Writer = <InputImpl as InputProps>::Writer;

    fn id(&self) -> &uuid::Uuid {
        &self.id
    }

    fn name(&self) -> &str {
        &self.desc.name
    }

    fn desc(&self) -> &BlockDesc {
        // Natural `&self` borrow. Earlier versions returned an
        // `unsafe { &*local } as &'static BlockDesc` to satisfy a
        // trait that wanted `&'static`; the trait was relaxed to
        // `&BlockDesc` so the unsafe extension is no longer needed.
        &self.desc
    }

    fn set_state(&mut self, state: BlockState) {
        self.state = state;
    }

    fn state(&self) -> &BlockState {
        &self.state
    }

    fn get_input(
        &self,
        name: &str,
    ) -> Option<&(dyn crate::base::block::BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs
            .iter()
            .find(|input| input.name() == name)
            .map(|input| input as _)
    }

    fn get_input_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut (dyn crate::base::block::BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs
            .iter_mut()
            .find(|input| input.name() == name)
            .map(|input| input as _)
    }

    fn get_output(
        &self,
        name: &str,
    ) -> Option<&(dyn crate::base::block::BlockOutput<Self::Writer> + Send)> {
        self.outputs
            .iter()
            .find(|output| output.name() == name)
            .map(|output| output as _)
    }

    fn get_output_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut (dyn crate::base::block::BlockOutput<Self::Writer> + Send)> {
        self.outputs
            .iter_mut()
            .find(|output| output.name() == name)
            .map(|output| output as _)
    }

    fn inputs(
        &self,
    ) -> Vec<&(dyn crate::base::block::BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs.iter().map(|input| input as _).collect()
    }

    fn inputs_mut(
        &mut self,
    ) -> Vec<&mut (dyn crate::base::block::BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs.iter_mut().map(|input| input as _).collect()
    }

    fn links(&self) -> Vec<(&str, Vec<&(dyn Link + Send)>)> {
        let mut res = Vec::new();

        self.inputs()
            .iter()
            .for_each(|input| res.push((input.name(), input.links())));
        self.outputs()
            .iter()
            .for_each(|out| res.push((out.name(), out.links())));
        res
    }

    fn outputs(&self) -> Vec<&(dyn crate::base::block::BlockOutput<Self::Writer> + Send)> {
        self.outputs.iter().map(|output| output as _).collect()
    }

    fn outputs_mut(
        &mut self,
    ) -> Vec<&mut (dyn crate::base::block::BlockOutput<Self::Writer> + Send)> {
        self.outputs.iter_mut().map(|output| output as _).collect()
    }

    fn remove_all_links(&mut self) {
        self.inputs
            .iter_mut()
            .for_each(|input| input.remove_all_links());

        self.outputs
            .iter_mut()
            .for_each(|output| output.remove_all_links());
    }

    fn remove_link(&mut self, link: &dyn Link) {
        self.inputs
            .iter_mut()
            .for_each(|input| input.remove_link(link));

        self.outputs
            .iter_mut()
            .for_each(|output| output.remove_link(link));
    }

    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.inputs
            .iter_mut()
            .for_each(|input| input.remove_link_by_id(link_id));

        self.outputs
            .iter_mut()
            .for_each(|output| output.remove_link_by_id(link_id));
    }
}

impl Block for JsBlock {
    async fn execute(&mut self) {
        // A reaction held from a cancelled cycle is retried before
        // anything else: the actor drops the in-flight `execute` future
        // whenever a mailbox command arrives, and the drop can land on
        // the `JsFuture` await inside `call_js_function` — after the
        // input wait already consumed the watch channels. Waiting on
        // inputs again there would park until *new* input arrives,
        // silently discarding the drained reaction. So only wait when
        // no reaction is owed; a held reaction skips straight to the
        // call. This applies to the polling (`Always`) variant too —
        // it would eventually re-fire on its own, but the retry saves
        // the reaction from waiting out another sleep window.
        if self.pending {
            // Drain, without blocking, anything that arrived while the
            // reaction was held, so the retry below reads the freshest
            // caches — a fresh value supersedes the held reaction
            // rather than queueing a second call behind it.
            drain_ready_inputs(self);
        } else {
            if let Some(BlockRunCondition::Always) = self.desc.run_condition {
                self.wait_on_inputs(Duration::from_millis(scan_interval_millis()))
                    .await;
            } else {
                self.read_inputs_until_ready().await;
            }
            self.pending = true;
        }

        let values = self
            .inputs
            .iter()
            .map(|input| input.value())
            .collect::<Vec<_>>();

        match to_js_value(&values) {
            Ok(values) => {
                self.call_js_function(values).await;
            }
            Err(err) => {
                log::debug!("Failed to serialize input values: {err}");
                self.set_state(BlockState::fault(format!(
                    "JS input serialization failed: {err}"
                )));
            }
        }

        // Reached only when the cycle ran to completion — a
        // cancellation drops this future at the `JsFuture` await inside
        // `call_js_function` and never gets here. Success and fault
        // alike consume the reaction; only a cancellation keeps it.
        self.pending = false;
    }
}

pub(crate) fn schedule_js_block(
    engine: &mut impl BlockSink,
    desc: &BlockDesc,
    block_id: Option<Uuid>,
) -> Result<Uuid> {
    let func = resolve_js_execute_function(desc)?;

    let block = JsBlock::new(desc.clone(), func, block_id);
    let id = *block.id();

    engine.accept(block)?;

    Ok(id)
}

pub(crate) async fn eval_js_block(desc: &BlockDesc, inputs: Vec<Value>) -> Result<Vec<Value>> {
    let func = resolve_js_execute_function(desc)?;

    let mut block = JsBlock::new(desc.clone(), func, None);
    eval_block_impl(&mut block, inputs).await
}

/// The function executing a block of type `desc`: what its factory
/// returns, or `None` for a block registered without a factory, which
/// is a no-op.
///
/// A block `registerBlock` never registered — an external block a Rust
/// application put in the registry — has no executor, and is rejected
/// with [`ExternalError::LibraryNotFound`].
fn resolve_js_execute_function(
    desc: &BlockDesc,
) -> Result<Option<js_sys::Function>, ExternalError> {
    JS_FNS.with(|fns| {
        let fns = fns.borrow();
        let factory = fns
            .get(&desc.library)
            .and_then(|lib| lib.get(desc.name.as_str()));

        let Some(factory) = factory else {
            return Err(ExternalError::LibraryNotFound {
                library: desc.library.clone(),
                name: desc.name.clone(),
            });
        };
        let Some(factory) = factory else {
            return Ok(None);
        };

        let func =
            factory
                .call0(&JsValue::NULL)
                .map_err(|err| ExternalError::FactoryCallFailed {
                    name: desc.name.clone(),
                    detail: js_detail(&err),
                })?;

        let func = func.dyn_into::<js_sys::Function>().map_err(|func| {
            ExternalError::FactoryReturnedNonFunction {
                name: desc.name.clone(),
                detail: format!("it returned a value of type {}", js_type_name(&func)),
            }
        })?;

        Ok(Some(func))
    })
}
