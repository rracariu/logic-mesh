// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::util::execute_impl;

/// Outputs true if value of the inputs are not equal.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct NotEqual {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for NotEqual {
    async fn execute(&mut self) {
        execute_impl(self, |in1, in2| in1 != in2).await;
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::NotEqual,
    };

    #[tokio::test]
    async fn test_neq_block() {
        let mut block = NotEqual::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input1, ("true").into()),
            (&mut block.input2, ("false").into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
