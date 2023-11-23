// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Gets the element specified at key from the input and outputs the
/// element's value.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
pub struct GetElement {
    #[input(name = "input", kind = "Null")]
    pub input: InputImpl,
    #[input(name = "key", kind = "Null")]
    pub key: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
}

impl Block for GetElement {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Dict(dict)) = self.input.get_value() {
            if let Some(Value::Str(key)) = self.key.get_value() {
                if let Some(value) = dict.get(key.as_str()) {
                    self.out.set(value.clone());
                }
            }
        } else if let Some(Value::List(list)) = self.input.get_value() {
            if let Some(Value::Number(index)) = self.key.get_value() {
                if let Some(value) = list.get(index.value as usize) {
                    self.out.set(value.clone());
                }
            }
        }
    }
}
