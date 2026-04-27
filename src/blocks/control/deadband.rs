// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_matching};

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Deadband (Schmitt-trigger) block. Output turns ON when `in` rises at or
/// above `high`, and turns OFF when `in` falls at or below `low`. Between
/// the two thresholds the output holds its previous state, preventing
/// short-cycling when a sensor hovers around a setpoint.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Deadband {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub high: InputImpl,
    #[input(kind = "Number")]
    pub low: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Deadband {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let in_n = match input_as_number(&self.input) {
            Some(n) => n,
            None => return,
        };
        let input = in_n.value;
        let high = match input_as_number_matching(&self.high, in_n.unit) {
            Some(v) => v,
            None => return,
        };
        let low = match input_as_number_matching(&self.low, in_n.unit) {
            Some(v) => v,
            None => return,
        };

        let current = matches!(&self.out.value, Value::Bool(b) if b.value);

        let next = if input >= high {
            true
        } else if input <= low {
            false
        } else {
            current
        };

        self.out.set(Bool { value: next }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        base::input::input_reader::InputReader, blocks::control::Deadband,
    };

    async fn write_all(block: &mut Deadband, inp: f64, low: f64, high: f64) {
        for _ in write_block_inputs(&mut [
            (&mut block.input, inp.into()),
            (&mut block.low, low.into()),
            (&mut block.high, high.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
    }

    #[tokio::test]
    async fn test_deadband_above_high_turns_on() {
        let mut block = Deadband::new();
        write_all(&mut block, 80.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_deadband_below_low_turns_off() {
        let mut block = Deadband::new();
        write_all(&mut block, 50.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_deadband_holds_in_window() {
        let mut block = Deadband::new();

        // Drive ON
        write_all(&mut block, 80.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Inside the deadband window — must hold previous (true)
        write_all(&mut block, 70.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Cross below low — turns off
        write_all(&mut block, 55.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, false.into());

        // Back inside the deadband — holds false
        write_all(&mut block, 70.0, 60.0, 75.0).await;
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
