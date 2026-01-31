// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Performs an subtraction of 2 numbers.
/// The operation would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[category = "math"]
pub struct Sub {
    #[input(kind = "Number")]
    pub a: InputImpl,
    #[input(kind = "Number")]
    pub b: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Sub {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Number(a)), Some(Value::Number(b))) =
            (&self.a.get_value(), &self.b.get_value())
        {
            match *a - *b {
                Ok(res) => self.out.set(res.into()),
                Err(e) => {
                    log::error!("Error while subtracting: {}", e);
                    self.set_state(BlockState::Fault);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::math::Sub,
    };

    #[tokio::test]
    async fn test_sub_block() {
        let mut block = Sub::new();

        for _ in
            write_block_inputs(&mut [(&mut block.a, 10.into()), (&mut block.b, 3.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, 7.into());
    }
}
