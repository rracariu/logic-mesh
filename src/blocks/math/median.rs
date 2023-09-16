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

/// Calculates the median of multiple numbers from the 16 inputs
/// this block has.
/// The operation would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Average"]
#[category = "math"]
#[input(kind = "Number", count = 16)]
pub struct Median {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Median {
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

        if let Ok(mut numbers) = convert_units(&val) {
            if self.state() != BlockState::Running {
                self.set_state(BlockState::Running);
            }

            numbers.sort();

            let median = if numbers.len() % 2 == 0 {
                let mid = numbers.len() / 2;
                (numbers[mid].value + numbers[mid - 1].value) / 2.0
            } else {
                let mid = numbers.len() / 2;
                numbers[mid].value
            };

            let median = if let Some(Number {
                value: _,
                unit: Some(unit),
            }) = numbers.get(0)
            {
                Number::make_with_unit(median, unit)
            } else {
                Number::make(median)
            };

            self.out.set((median).into());
        } else {
            self.set_state(BlockState::Fault);
        }
    }
}

#[cfg(test)]
mod test {

    use crate::base::block::test_utils::write_block_inputs;
    use crate::base::input::input_reader::InputReader;
    use crate::{base::block::Block, blocks::math::Median};

    #[tokio::test]
    async fn test_median_block() {
        let mut block = Median::new();

        write_block_inputs(&mut [(&mut block._inputs.get_mut(0).unwrap(), 1.into())]).await;
        block.read_inputs().await;
        write_block_inputs(&mut [(&mut block._inputs.get_mut(15).unwrap(), 9.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 5.into());

        write_block_inputs(&mut [(&mut block._inputs.get_mut(1).unwrap(), 1.into())]).await;
        block.read_inputs().await;
        write_block_inputs(&mut [(&mut block._inputs.get_mut(2).unwrap(), 2.into())]).await;
        block.read_inputs().await;
        write_block_inputs(&mut [(&mut block._inputs.get_mut(3).unwrap(), 3.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 2.into());
    }
}
