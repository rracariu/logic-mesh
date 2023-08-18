// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Returns the Tangent value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Tangent"]
#[category = "math"]
pub struct Tan {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Tan {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(a)) = self.input.get_value() {
            self.out.set(
                Number {
                    value: a.value.tan(),
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
        blocks::math::Tan,
    };

    #[tokio::test]
    async fn test_tan_block() {
        let mut block = Tan::new();

        for _ in write_block_inputs(&mut [(&mut block.input, 0.into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value.round() == 0.0
        );
    }
}
