// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the input value if the condition is true.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Latch {
    #[input(name = "in", kind = "Null")]
    pub input: InputImpl,
    #[input(kind = "Bool")]
    pub condition: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
}

impl Block for Latch {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let input = self.input.get_value();
        let condition = self.condition.get_value();

        if let (Some(Value::Bool(condition)), Some(input)) = (condition, input) {
            if condition.value {
                self.out.set(input.clone());
            } else {
                self.out.set(Value::Null);
            }
        }
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::logic::latch::Latch,
    };

    #[tokio::test]
    async fn test_latch_false() {
        let mut block = Latch::new();

        write_block_inputs([(&mut block.input, 42), (&mut block.condition, false.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, Value::Null);
    }

    #[tokio::test]
    async fn test_latch_true() {
        let mut block = Latch::new();

        write_block_inputs([(&mut block.input, 42), (&mut block.condition, true.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 42.into());
    }
}
