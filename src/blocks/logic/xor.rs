// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Bool, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Outputs the logical Xor value of the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Xor {
    #[input(name = "in1", kind = "Bool")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Bool")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Xor {
    async fn execute(&mut self) {
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        if let (Some(Value::Bool(a)), Some(Value::Bool(b))) =
            (self.input1.get_value(), self.input2.get_value())
        {
            self.out.set(
                Bool {
                    value: a.value ^ b.value,
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::Xor,
    };

    #[tokio::test]
    async fn test_xor_block() {
        let mut block = Xor::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input1, (true).into()),
            (&mut block.input2, (1).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
