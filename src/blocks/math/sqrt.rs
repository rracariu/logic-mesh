// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the square root value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "SquareRoot"]
#[category = "math"]
pub struct Sqrt {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Sqrt {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.input.get_value() {
            self.out.set(
                Number {
                    value: a.value.sqrt(),
                    unit: a.unit,
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {

    use std::assert_matches::assert_matches;

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::math::Sqrt,
    };

    #[tokio::test]
    async fn test_sqrt_block() {
        let mut block = Sqrt::new();

        write_block_inputs(&mut [(&mut block.input, 4.into())]).await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 2.0
        );
    }
}
