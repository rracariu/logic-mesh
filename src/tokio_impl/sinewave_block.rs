// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::kind::HaystackKind;
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use super::{input::InputImpl, output::OutputImpl};

#[derive(BlockProps)]
#[name = "SineWave"]
#[library = "math"]
pub struct SineWave {
    id: Uuid,
    pub period: InputImpl,
    pub out: OutputImpl,
}

impl Block for SineWave {
    async fn execute(&mut self) {}
}

impl SineWave {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            period: InputImpl::new("a", HaystackKind::Number),
            out: OutputImpl::new(HaystackKind::Number),
        }
    }
}
