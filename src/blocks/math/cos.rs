// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the cosine value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "math"]
pub struct Cos {
    #[input(name = "in", kind = "Number")]
    pub a: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Cos {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.a.get_value() {
            self.out.set(
                Number {
                    value: a.value.cos(),
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
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::math::Cos,
    };

    #[tokio::test]
    async fn test_cos_block() {
        let mut block = Cos::new();

        for _ in write_block_inputs(&mut [(&mut block.a, 0.into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value.round() == 1.0
        );
    }
}
