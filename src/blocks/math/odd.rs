// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs true if the input value is odd.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Odd"]
#[category = "math"]
pub struct Odd {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Odd {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.input.get_value() {
            let is_odd = a.value % 2.0 != 0.0;
            self.out.set(is_odd.into());
        }
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use libhaystack::val::{Bool, Value};

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::math::Odd,
    };

    #[tokio::test]
    async fn test_odd_block() {
        let mut block = Odd::new();

        write_block_inputs(&mut [(&mut block.input, 8.into())]).await;
        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Bool(Bool { value, .. }) if value == false
        );

        write_block_inputs(&mut [(&mut block.input, 9.into())]).await;
        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Bool(Bool { value, .. }) if value
        );
    }
}
