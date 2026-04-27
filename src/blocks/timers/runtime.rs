// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::get_sleep_dur;
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::units::units_generated::HOUR;
use libhaystack::val::{Number, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Runtime accumulator. Integrates the time `in` is true and exposes the
/// total as hours on the `hours` output. A rising edge on `reset` zeros
/// the accumulator. Useful for tracking equipment runtime for
/// maintenance scheduling.
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct Runtime {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Bool")]
    pub reset: InputImpl,
    #[output(name = "hours", kind = "Number")]
    pub out: OutputImpl,
    last_tick_ms: u64,
    accumulated_ms: u64,
    prev_reset: Option<bool>,
}

impl Block for Runtime {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        let on = matches!(self.input.get_value(), Some(Value::Bool(b)) if b.value);
        let reset = matches!(self.reset.get_value(), Some(Value::Bool(b)) if b.value);
        let now = current_time_millis();

        // Reset on rising edge of `reset` so the accumulator clears once,
        // not continuously while reset is held.
        let rising_reset = matches!(self.prev_reset, Some(false)) && reset;
        self.prev_reset = Some(reset);

        if rising_reset {
            self.accumulated_ms = 0;
            self.last_tick_ms = now;
            self.out.set(Number::make_with_unit(0.0, &HOUR).into());
            return;
        }

        if self.last_tick_ms == 0 {
            self.last_tick_ms = now;
        }

        if on {
            let dt = now.saturating_sub(self.last_tick_ms);
            self.accumulated_ms = self.accumulated_ms.saturating_add(dt);
        }
        self.last_tick_ms = now;

        let hours = self.accumulated_ms as f64 / 3_600_000.0;
        self.out.set(Number::make_with_unit(hours, &HOUR).into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::{units::units_generated::HOUR, val::Value};

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::timers::Runtime,
    };

    fn link_out(block: &mut Runtime) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_runtime_starts_at_zero() {
        let mut block = Runtime::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => {
                assert_eq!(n.value, 0.0);
                assert_eq!(n.unit, Some(&HOUR));
            }
            _ => panic!("expected Number"),
        }
    }

    #[tokio::test]
    async fn test_runtime_reset_clears_accumulator() {
        let mut block = Runtime::new();
        link_out(&mut block);

        // Simulate some accumulated time
        block.accumulated_ms = 3_600_000; // 1h

        // Establish reset=false baseline
        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Rising edge of reset → clears
        for _ in write_block_inputs(&mut [(&mut block.reset, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.accumulated_ms, 0);
        assert!(matches!(block.out.value, Value::Number(n) if n.value == 0.0));
    }

    #[tokio::test]
    async fn test_runtime_off_does_not_accumulate() {
        let mut block = Runtime::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        let first = block.accumulated_ms;
        block.execute().await;
        let second = block.accumulated_ms;
        assert_eq!(first, second);
    }
}
