// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Bool};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

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
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        self.out.set(
            Bool {
                value: self.input1.get_value() >= self.input2.get_value(),
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
        blocks::logic::GreaterThanEq,
    };

    #[tokio::test]
    async fn test_gte_block() {
        let mut block = GreaterThanEq::new();

        for _ in
            write_block_inputs(&mut [(&mut block.input1, 3.into()), (&mut block.input2, 3.into())])
                .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
