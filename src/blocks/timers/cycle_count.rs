// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Counts rising edges on `in`. Useful for tracking equipment start
/// counts for maintenance and wear tracking. A rising edge on `reset`
/// zeros the counter.
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct CycleCount {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Bool")]
    pub reset: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    count: u64,
    prev_input: Option<bool>,
    prev_reset: Option<bool>,
}

impl Block for CycleCount {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let input = matches!(self.input.get_value(), Some(Value::Bool(b)) if b.value);
        let reset = matches!(self.reset.get_value(), Some(Value::Bool(b)) if b.value);

        let rising_reset = matches!(self.prev_reset, Some(false)) && reset;
        self.prev_reset = Some(reset);

        if rising_reset {
            self.count = 0;
        }

        let rising_input = matches!(self.prev_input, Some(false)) && input;
        self.prev_input = Some(input);

        if rising_input {
            self.count = self.count.saturating_add(1);
        }

        self.out.set((self.count as f64).into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::timers::CycleCount,
    };

    #[tokio::test]
    async fn test_cycle_count_increments_on_rising_edge() {
        let mut block = CycleCount::new();

        // Establish low
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (0.0_f64).into());

        // Rising edge → count = 1
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (1.0_f64).into());

        // Holding true does not double-count
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (1.0_f64).into());

        // Falling, then rising again → count = 2
        for _ in write_block_inputs(&mut [(&mut block.input, false.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (2.0_f64).into());
    }

    #[tokio::test]
    async fn test_cycle_count_reset() {
        let mut block = CycleCount::new();
        block.count = 42;

        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        for _ in write_block_inputs(&mut [(&mut block.reset, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.count, 0);
    }
}
