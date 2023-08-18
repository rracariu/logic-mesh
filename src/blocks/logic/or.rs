// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Bool, Value};

use crate::{
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Outputs the logical Or value of the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Or {
    #[input(name = "in1", kind = "Bool")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Bool")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Or {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Bool(a)), Some(Value::Bool(b))) =
            (self.input1.get_value(), self.input2.get_value())
        {
            self.out.set(
                Bool {
                    value: a.value || b.value,
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
        blocks::logic::Or,
    };

    #[tokio::test]
    async fn test_or_block() {
        let mut block = Or::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input1, (true).into()),
            (&mut block.input2, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
