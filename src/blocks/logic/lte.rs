// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Bool};

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::binary::BinaryBlock;

/// Outputs true if value of the in1 is less or equal.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "LessThanEqual"]
#[category = "logic"]
pub struct LessThanEq {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl BinaryBlock for LessThanEq {}

impl Block for LessThanEq {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let (input1, input2) = self.convert_inputs();

        self.out.set(
            Bool {
                value: input1 <= input2,
            }
            .into(),
        );
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::LessThanEq,
    };

    #[tokio::test]
    async fn test_lte_block() {
        let mut block = LessThanEq::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input1, 41.into()),
            (&mut block.input2, 41.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
