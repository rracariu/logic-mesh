// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Returns the cosinus value of the input.
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
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

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

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::math::Cos,
    };

    #[tokio::test]
    async fn test_sub() {
        let mut block = Cos::new();

        for _ in write_block_inputs(&mut [(&mut block.a, 0.into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;

        assert!(matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value.round() == 1.0
        ));
    }
}
