// Copyright (c) 2022-2023, Radu Racariu.

use anyhow::Ok;
use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps, input_reader::InputReader},
        output::Output,
    },
    blocks::utils::convert_units,
};

use libhaystack::val::{Number, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the modulus value of the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Modulus"]
#[category = "math"]
pub struct Mod {
    #[input(kind = "Number")]
    pub a: InputImpl,
    #[input(kind = "Number")]
    pub b: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Mod {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Number(a)), Some(Value::Number(b))) =
            (self.a.get_value(), self.b.get_value())
        {
            let _ = convert_units(&[a.to_owned(), b.to_owned()])
                .and_then(|parts| {
                    if let [a, b] = &parts[..] {
                        let val = Number {
                            value: a.value % b.value,
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
        blocks::math::Mod,
    };

    #[tokio::test]
    async fn test_mod_block() {
        let mut block = Mod::new();

        for _ in write_block_inputs(&mut [(&mut block.a, 6.into()), (&mut block.b, 4.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 2.0
        );
    }
}
