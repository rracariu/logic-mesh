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

/// Off-delay timer. Output goes true immediately when `in` becomes true,
/// and stays true for at least `delay` (milliseconds) after `in` goes
/// false. Any new true on `in` during the delay window cancels the
/// pending drop.
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct OffDelay {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub delay: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
    falling_since_ms: u64,
}

impl Block for OffDelay {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        let input = matches!(self.input.get_value(), Some(Value::Bool(b)) if b.value);
        let delay_ms = input_to_millis_or_default(&self.delay.val);
        let now = current_time_millis();

        if input {
            self.falling_since_ms = 0;
            self.out.set(Bool { value: true }.into());
            return;
        }

        // input is false
        if self.falling_since_ms == 0 {
            // Only start the off-delay if the output was on; otherwise
            // a fresh block with a false input should just stay off.
            if matches!(&self.out.value, Value::Bool(b) if b.value) {
                self.falling_since_ms = now;
            } else {
                self.out.set(Bool { value: false }.into());
                return;
            }
        }

        let elapsed = now.saturating_sub(self.falling_since_ms);
        let still_on = elapsed < delay_ms;
        if !still_on {
            self.falling_since_ms = 0;
        }
        self.out.set(Bool { value: still_on }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, link::BaseLink},
        blocks::timers::OffDelay,
    };

    #[tokio::test]
    async fn test_off_delay_input_true_passes_through() {
        let mut block = OffDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        write_block_inputs([(&mut block.input, true.into()), (&mut block.delay, 1000)]).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_off_delay_holds_output_after_input_drops() {
        let mut block = OffDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Drive the output on
        write_block_inputs([
            (&mut block.input, true.into()),
            (&mut block.delay, 3_600_000),
        ])
        .await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Drop input — output should hold true (1 hour delay)
        write_block_inputs([
            (&mut block.input, false.into()),
            (&mut block.delay, 3_600_000),
        ])
        .await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());
        assert!(block.falling_since_ms > 0);
    }

    #[tokio::test]
    async fn test_off_delay_zero_delay_drops_immediately() {
        let mut block = OffDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Drive on
        write_block_inputs([(&mut block.input, true.into()), (&mut block.delay, 0)]).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Drop input with zero delay — falls immediately
        write_block_inputs([(&mut block.input, false.into()), (&mut block.delay, 0)]).await;
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_off_delay_initial_false_stays_false() {
        let mut block = OffDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Fresh block + false input — must stay off without arming the timer.
        write_block_inputs([
            (&mut block.input, false.into()),
            (&mut block.delay, 3_600_000),
        ])
        .await;
        block.execute().await;
        assert_eq!(block.out.value, false.into());
        assert_eq!(block.falling_since_ms, 0);
    }

    #[tokio::test]
    async fn test_off_delay_retrigger_cancels_drop() {
        let mut block = OffDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // On, then off (arms timer), then on again (cancels)
        write_block_inputs([
            (&mut block.input, true.into()),
            (&mut block.delay, 3_600_000),
        ])
        .await;
        block.execute().await;

        write_block_inputs([(&mut block.input, false)]).await;
        block.execute().await;
        assert!(block.falling_since_ms > 0);

        write_block_inputs([(&mut block.input, true)]).await;
        block.execute().await;
        assert_eq!(block.out.value, true.into());
        assert_eq!(block.falling_since_ms, 0);
    }
}
