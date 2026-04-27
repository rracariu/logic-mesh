// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{get_sleep_dur, input_to_millis_or_default};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// One-shot pulse. On a rising edge of `in`, the output goes true for
/// `width` milliseconds and then back to false. Subsequent rising edges
/// while the pulse is still active are ignored (non-retriggerable).
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct OneShot {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub width: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
    pulse_started_ms: u64,
    prev: Option<bool>,
}

impl Block for OneShot {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        let current = matches!(self.input.get_value(), Some(Value::Bool(b)) if b.value);
        let width_ms = input_to_millis_or_default(&self.width.val);
        let now = current_time_millis();

        let rising = matches!(self.prev, Some(false)) && current;
        self.prev = Some(current);

        if rising && self.pulse_started_ms == 0 {
            self.pulse_started_ms = now;
        }

        let active = if self.pulse_started_ms > 0 {
            let elapsed = now.saturating_sub(self.pulse_started_ms);
            if elapsed >= width_ms {
                self.pulse_started_ms = 0;
                false
            } else {
                true
            }
        } else {
            false
        };

        self.out.set(Bool { value: active }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::timers::OneShot,
    };

    fn link_out(block: &mut OneShot) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_one_shot_first_cycle_no_pulse() {
        let mut block = OneShot::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.width, (1000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        // No prior false → no rising edge yet
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_one_shot_fires_on_rising_edge() {
        let mut block = OneShot::new();
        link_out(&mut block);

        // Establish low
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.width, (3_600_000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Rising edge → pulse active for 1h
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_one_shot_zero_width_never_pulses() {
        let mut block = OneShot::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.width, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        // width=0 → elapsed >= width immediately → pulse closes same cycle
        assert_eq!(block.out.value, false.into());
    }
}
