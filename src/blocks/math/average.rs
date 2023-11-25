// Copyright (c) 2022-2023, Radu Racariu.

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

/// Calculates an average of multiple numbers from the 16 inputs
/// this block has.
/// The operation would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Average"]
#[category = "math"]
#[input(kind = "Number", count = 16)]
pub struct Average {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Average {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let val = self
            .inputs()
            .into_iter()
            .filter_map(|input| match input.get_value().as_ref() {
                Some(Value::Number(num)) => Some(*num),
                _ => None,
            })
            .collect::<Vec<Number>>();

        if let Ok(numbers) = convert_units(&val) {
            if self.state() != BlockState::Running {
                self.set_state(BlockState::Running);
            }

            let avg = numbers.iter().fold(0.0, |acc, n| acc + n.value) / numbers.len() as f64;

            let avg = if let Some(Number {
                value: _,
                unit: Some(unit),
            }) = numbers.first()
            {
                Number::make_with_unit(avg, unit)
            } else {
                Number::make(avg)
            };

            self.out.set((avg).into())
        } else {
            self.set_state(BlockState::Fault);
        }
    }
}

#[cfg(test)]
mod test {

    use crate::base::block::test_utils::write_block_inputs;
    use crate::base::input::input_reader::InputReader;
    use crate::{base::block::Block, blocks::math::Average};

    #[tokio::test]
    async fn test_average_block() {
        let mut block = Average::new();

        write_block_inputs(&mut [(&mut block._inputs.get_mut(0).unwrap(), 1.into())]).await;
        block.read_inputs().await;
        write_block_inputs(&mut [(&mut block._inputs.get_mut(15).unwrap(), 9.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 5.into());
    }
}
