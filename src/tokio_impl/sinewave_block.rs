// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::kind::HaystackKind;
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use super::{input::InputImpl, output::OutputImpl};

#[block]
#[derive(BlockProps, Debug)]
#[name = "SineWave"]
#[library = "math"]
#[input(kind = Number, 16)]
pub struct SineWave {
    #[input(kind = Number)]
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
            state: BlockState::Stopped,
            period: InputImpl::new("a", HaystackKind::Number),
            out: OutputImpl::new(HaystackKind::Number),
        }
    }
}
