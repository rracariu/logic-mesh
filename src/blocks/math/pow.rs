// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the power root value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Power"]
#[category = "math"]
pub struct Pow {
    #[input(name = "base", kind = "Number")]
    pub base: InputImpl,
    #[input(name = "exponent", kind = "Number")]
    pub exponent: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Pow {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Number(base)), Some(Value::Number(exponent))) =
            (self.base.get_value(), self.exponent.get_value())
        {
            self.out.set(
                Number {
                    value: base.value.powf(exponent.value),
                    unit: base.unit,
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
        blocks::math::Pow,
    };

    #[tokio::test]
    async fn test_pow_block() {
        let mut block = Pow::new();

        for _ in
            write_block_inputs(&mut [(&mut block.base, 4.into()), (&mut block.exponent, 4.into())])
                .await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 256.0
        );
    }
}
