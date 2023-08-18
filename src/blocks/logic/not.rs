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

/// Outputs the negated value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Not {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Not {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Bool(a)) = self.input.get_value() {
            self.out.set(Bool { value: !a.value }.into());
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::Not,
    };

    #[tokio::test]
    async fn test_not_block() {
        let mut block = Not::new();

        for _ in write_block_inputs(&mut [(&mut block.input, (true).into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
