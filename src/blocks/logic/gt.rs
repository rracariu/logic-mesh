// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Bool};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs true if value of the in1 is greater.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "GreaterThan"]
#[category = "logic"]
pub struct GreaterThan {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for GreaterThan {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        self.out.set(
            Bool {
                value: self.input1.get_value() > self.input2.get_value(),
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
        blocks::logic::GreaterThan,
    };

    #[tokio::test]
    async fn test_gt_block() {
        let mut block = GreaterThan::new();

        for _ in
            write_block_inputs(&mut [(&mut block.input1, 3.into()), (&mut block.input2, 1.into())])
                .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
