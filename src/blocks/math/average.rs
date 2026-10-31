// Copyright (c) 2022-2023, Radu Racariu.

//! Average block.

use crate::{
    base::{
        block::{Block, BlockProps, BlockState},
        input::{InputProps, InputReader},
        output::Output,
    },
    blocks::utils::{common_unit, to_unit},
};

use libhaystack::val::{Number, Value};

use crate::blocks::OutputImpl;

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

        let numbers = || {
            self._inputs.iter().filter_map(|input| match input.value() {
                Some(Value::Number(num)) => Some(*num),
                _ => None,
            })
        };
        let unit = common_unit(numbers());
        let sum = numbers().try_fold((0.0, 0usize), |(sum, count), num| {
            to_unit(num, unit).map(|num| (sum + num.value, count + 1))
        });

        if let Ok((sum, count)) = sum {
            if !matches!(self.state(), BlockState::Running) {
                self.set_state(BlockState::Running);
            }

            let avg = sum / count as f64;

            let avg = if let Some(unit) = unit {
                Number::make_with_unit(avg, unit)
            } else {
                Number::make(avg)
            };

            self.out.set((avg).into())
        } else {
            self.set_state(BlockState::fault("Average: unit conversion failed"));
        }
    }
}

#[cfg(test)]
mod test {

    use crate::base::block::test_utils::write_block_inputs;
    use crate::{base::block::Block, base::input::InputReader, blocks::math::Average};

    #[tokio::test]
    async fn test_average_block() {
        let mut block = Average::new();

        write_block_inputs([(block._inputs.get_mut(0).unwrap(), 1)]).await;
        block.read_inputs().await;
        write_block_inputs([(block._inputs.get_mut(15).unwrap(), 9)]).await;

        block.execute().await;
        assert_eq!(block.out.value, 5.into());
    }
}
