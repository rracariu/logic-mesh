// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::kind::HaystackKind;

use super::{input::InputImpl, output::OutputImpl};

#[block]
#[derive(BlockProps, Debug)]
#[name = "SineWave"]
#[library = "math"]
#[input(kind = "Bool", count = 4)]
pub struct SineWave {
    #[input(kind = Number)]
    pub period: InputImpl,
    #[output(kind = Number)]
    pub out: OutputImpl,
}

impl Block for SineWave {
    async fn execute(&mut self) {}
}
