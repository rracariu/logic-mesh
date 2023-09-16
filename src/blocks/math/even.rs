// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs true if the input value is even.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Even"]
#[category = "math"]
pub struct Even {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Even {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.input.get_value() {
            let is_even = a.value % 2.0 == 0.0;
            self.out.set(is_even.into());
        }
    }
}

#[cfg(test)]
mod test {

    use std::assert_matches::assert_matches;

    use libhaystack::val::{Bool, Value};

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::math::Even,
    };

    #[tokio::test]
    async fn test_even_block() {
        let mut block = Even::new();

        write_block_inputs(&mut [(&mut block.input, 8.into())]).await;
        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Bool(Bool { value, .. }) if value
        );

        write_block_inputs(&mut [(&mut block.input, 7.into())]).await;
        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Bool(Bool { value, .. }) if value == false
        );
    }
}
