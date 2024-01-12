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

/// Outputs true if value of the in1 is greater or equals.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "GreaterThanEqual"]
#[category = "logic"]
pub struct GreaterThanEq {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for GreaterThanEq {
    async fn execute(&mut self) {
        execute_impl(self, |in1, in2| in1 >= in2).await;
    }
}
#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::GreaterThanEq,
    };

    #[tokio::test]
    async fn test_gte_block() {
        let mut block = GreaterThanEq::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input1, 3.into()),
            (&mut block.input2, (-3).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
