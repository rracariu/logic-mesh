// Copyright (c) 2022-2023, Radu Racariu.

use anyhow::Ok;
use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::Output,
    },
    blocks::utils::convert_units,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

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
            let _ = convert_units(&[a.to_owned(), b.to_owned()])
                .and_then(|parts| {
                    if let [a, b] = &parts[..] {
                        let val = Number {
                            value: a.value.max(b.value),
                            unit: a.unit,
                        };
                        self.out.set(val.into());
                    }
                    Ok(())
                })
                .or_else(|_| {
                    self.set_state(BlockState::Fault);
                    Ok(())
                });
        }
    }
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::math::Max,
    };

    #[tokio::test]
    async fn test_max_block() {
        let mut block = Max::new();

        for _ in
            write_block_inputs(&mut [(&mut block.a, 8.into()), (&mut block.b, 42.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 42.0
        );
    }
}
