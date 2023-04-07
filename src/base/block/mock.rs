// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines mock input and output types for testing
//!

use std::pin::Pin;

use uuid::Uuid;

use crate::base::{
    input::{BaseInput, Input, InputProps, InputReceiver},
    link::{BaseLink, Link},
    output::{BaseOutput, Output},
};
use libhaystack::val::{kind::HaystackKind, Value};

pub type InputImpl = BaseInput<String, String, BaseLink<String>>;

impl InputImpl {
    pub fn new(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
        Self {
            name: name.to_string(),
            kind,
            block_id,
            ..Default::default()
        }
    }
}

impl Input for InputImpl {
    fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>> {
        Box::pin(async { None })
    }
}

pub type OutputImpl = BaseOutput<BaseLink<String>>;

impl Default for OutputImpl {
    fn default() -> Self {
        Self::new(HaystackKind::Null, Uuid::default())
    }
}

impl Output for OutputImpl {
    type Tx = <InputImpl as InputProps>::Tx;
    fn add_link(&mut self, _link: BaseLink<Self::Tx>) {}

    fn remove_link(&mut self, _link: &dyn Link) {}

    fn set(&mut self, _value: Value) {}
}
