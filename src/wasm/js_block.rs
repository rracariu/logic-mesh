// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Result;
use js_sys::Promise;
use libhaystack::val::Value;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::base::block::desc::BlockRunCondition;
use crate::base::block::{Block, BlockState, BlockStaticDesc};
use crate::base::engine::Engine;
use crate::base::input::input_reader::InputReader;

use crate::blocks::utils::DEFAULT_SLEEP_DUR;
use crate::{
    base::{
        block::{BlockDesc, BlockProps},
        input::{Input, InputProps},
        link::Link,
        output::{Output, OutputProps},
    },
    blocks::{InputImpl, OutputImpl},
};

type ExternFuncRegistryType = BTreeMap<String, js_sys::Function>;
pub static mut JS_FNS: ExternFuncRegistryType = ExternFuncRegistryType::new();

/// A block that is implemented in JavaScript.
/// The block will delegate the evaluation to a JS function that will be called with the inputs as arguments.
pub struct JsBlock {
    id: Uuid,
    desc: BlockDesc,
    inputs: Vec<InputImpl>,
    outputs: Vec<OutputImpl>,
    state: BlockState,
    func: Option<js_sys::Function>,
}

impl JsBlock {
    /// Create a new instance of a block
    pub fn new(desc: BlockDesc, func: Option<js_sys::Function>, block_id: Option<Uuid>) -> Self {
        let id = block_id.unwrap_or_else(|| uuid::Uuid::new_v4());

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
            state: BlockState::Stopped,
            func,
        }
    }

    // Call the JS function with the inputs as arguments
    async fn call_js_function(&mut self, values: JsValue) {
        if let Some(ref func) = self.func {
            match func.call1(&JsValue::NULL, &values) {
                Ok(result) => {
                    let promise = Promise::from(result);
                    let future = JsFuture::from(promise);

                    future
                        .await
                        .map_err(|err| {
                            serde_wasm_bindgen::from_value::<String>(err)
                                .unwrap_or_else(|err| format!("{err:#?}"))
                        })
                        .and_then(|res| {
                            if res.is_array() {
                                serde_wasm_bindgen::from_value::<Vec<Value>>(res)
                                    .and_then(|list| {
                                        list.iter().enumerate().for_each(|(index, res)| {
                                            if let Some(output) = self.outputs.get_mut(index) {
                                                output.set(res.clone());
                                            }
                                        });
                                        Ok(())
                                    })
                                    .map_err(|err| format!("{err:#?}"))
                            } else {
                                Ok(())
                            }
                        })
                        .unwrap_or_else(|err| {
                            log::error!("Failed to process the return of the JS function: {err}");
                            self.set_state(BlockState::Fault);
                        });
                }
                Err(err) => {
                    log::error!("Failed to execute JS function block: {err:#?}");
                    self.set_state(BlockState::Fault);
                }
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

    fn desc(&self) -> &'static BlockDesc {
        let local: *const BlockDesc = &self.desc;
        unsafe { &*local }
    }

    fn set_state(&mut self, state: BlockState) -> BlockState {
        self.state = state;
        self.state
    }

    fn state(&self) -> BlockState {
        self.state
    }

    fn get_input(
        &self,
        name: &str,
    ) -> Option<&dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs
            .iter()
            .find(|input| input.name() == name)
            .map(|input| input as _)
    }

    fn get_input_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs
            .iter_mut()
            .find(|input| input.name() == name)
            .map(|input| input as _)
    }

    fn get_output(&self, name: &str) -> Option<&dyn Output<Writer = Self::Writer>> {
        self.outputs
            .iter()
            .find(|output| output.name() == name)
            .map(|output| output as _)
    }

    fn get_output_mut(&mut self, name: &str) -> Option<&mut dyn Output<Writer = Self::Writer>> {
        self.outputs
            .iter_mut()
            .find(|output| output.name() == name)
            .map(|output| output as _)
    }

    fn inputs(&self) -> Vec<&dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs.iter().map(|input| input as _).collect()
    }

    fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs.iter_mut().map(|input| input as _).collect()
    }

    fn links(&self) -> Vec<(&str, Vec<&dyn Link>)> {
        let mut res = Vec::new();

        self.inputs()
            .iter()
            .for_each(|input| res.push((input.name(), input.links())));
        self.outputs()
            .iter()
            .for_each(|out| res.push((out.name(), out.links())));
        res
    }

    fn outputs(&self) -> Vec<&dyn Output<Writer = Self::Writer>> {
        self.outputs.iter().map(|output| output as _).collect()
    }

    fn outputs_mut(&mut self) -> Vec<&mut dyn Output<Writer = Self::Writer>> {
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

impl BlockStaticDesc for JsBlock {
    fn desc() -> &'static BlockDesc {
        unimplemented!()
    }
}

impl Block for JsBlock {
    async fn execute(&mut self) {
        if let Some(BlockRunCondition::Always) = self.desc.run_condition {
            self.wait_on_inputs(Duration::from_millis(DEFAULT_SLEEP_DUR))
                .await;
        } else {
            self.read_inputs_until_ready().await;
        }

        let values = self
            .inputs
            .iter()
            .map(|input| input.get_value())
            .collect::<Vec<_>>();

        match serde_wasm_bindgen::to_value(&values) {
            Ok(values) => {
                self.call_js_function(values).await;
            }
            Err(err) => {
                log::error!("Failed to serialize input values: {err}");
                self.set_state(BlockState::Fault);
            }
        }
    }
}

pub(crate) fn schedule_js_block(
    engine: &mut impl Engine<
        Reader = <InputImpl as InputProps>::Reader,
        Writer = <InputImpl as InputProps>::Writer,
    >,
    desc: &BlockDesc,
    block_id: Option<Uuid>,
) -> Result<Uuid> {
    let func = unsafe { JS_FNS.get(desc.name.as_str()) };

    let func = match func {
        Some(func) => {
            let func = func
                .call0(&JsValue::NULL)
                .map_err(|err| anyhow::anyhow!("Failed to call JS function: {err:#?}"))?;

            let func = func
                .dyn_into::<js_sys::Function>()
                .map_err(|err| anyhow::anyhow!("Value is not a function: {err:#?}"))?;

            Some(func)
        }
        None => None,
    };

    let block = JsBlock::new(desc.clone(), func, block_id);
    let id = *block.id();

    engine.schedule(block);

    Ok(id)
}
