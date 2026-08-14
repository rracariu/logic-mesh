// Copyright (c) 2022-2023, Radu Racariu.

use crate::{
    base::{
        block::{Block, BlockProps, BlockState},
        input::{InputProps, input_reader::InputReader},
        output::Output,
    },
    blocks::utils::convert_units,
};

use libhaystack::val::{Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the Maximum value of the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Maximum"]
#[category = "math"]
pub struct Max {
    #[input(kind = "Number")]
    pub a: InputImpl,
    #[input(kind = "Number")]
    pub b: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Max {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Number(a)), Some(Value::Number(b))) =
            (self.a.get_value(), self.b.get_value())
        {
            convert_units(&[a.to_owned(), b.to_owned()])
                .map(|parts| {
                    if let [a, b] = &parts[..] {
                        let val = Number {
                            value: a.value.max(b.value),
                            unit: a.unit,
                        };
                        self.out.set(val.into());
                    }
                })
                .unwrap_or_else(|_| {
                    self.set_state(BlockState::fault("Max: unit conversion failed"));
                });
        }
    }
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::math::Max,
    };

    #[tokio::test]
    async fn test_max_block() {
        let mut block = Max::new();

        write_block_inputs([(&mut block.a, 8), (&mut block.b, 42)]).await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 42.0
        );
    }
}
