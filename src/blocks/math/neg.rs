// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the unary minus value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Negative"]
#[category = "math"]
pub struct Neg {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Neg {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.input.get_value() {
            self.out.set(
                Number {
                    value: -a.value,
                    unit: a.unit,
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::math::Neg,
    };

    #[tokio::test]
    async fn test_neg_block() {
        let mut block = Neg::new();

        write_block_inputs(&mut [(&mut block.input, 42.into())]).await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == -42.0
        );
    }
}
