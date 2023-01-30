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
#[lib = "math"]
pub struct SineWave {
    id: Uuid,
    pub period: InputImpl,
    pub out: OutputImpl,
    desc: BlockDesc,
}

impl Block for SineWave {
    async fn execute(&mut self) {}
}

impl SineWave {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            desc: BlockDesc {
                name: name.into(),
                library: "".into(),
            },
            period: InputImpl::new("a", HaystackKind::Number),
            out: OutputImpl::new(HaystackKind::Number),
        }
    }
}
