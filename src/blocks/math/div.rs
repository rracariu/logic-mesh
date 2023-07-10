// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Performs an division of 2 numbers.
/// The operation would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Divide"]
#[category = "math"]
pub struct Div {
    #[input(kind = "Number")]
    pub a: InputImpl,
    #[input(kind = "Number")]
    pub b: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Div {
    async fn execute(&mut self) {
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        let res = self.a.get_value().clone().and_then(|val| {
            if let Value::Number(a) = val {
                self.b.get_value().clone().and_then(|val| {
                    if let Value::Number(b) = val {
                        return (a / b).ok();
                    }
                    None
                })
            } else {
                None
            }
        });

        if let Some(res) = res {
            self.out.set(res.into())
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
        blocks::math::Div,
    };

    #[tokio::test]
    async fn test_div_block() {
        let mut block = Div::new();

        for _ in
            write_block_inputs(&mut [(&mut block.a, 42.into()), (&mut block.b, 2.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value.round() == 21.0
        );
    }
}
