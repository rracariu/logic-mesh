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

/// On-delay timer. Output goes true only after `in` has been continuously
/// true for at least `delay` (milliseconds). When `in` goes false the
/// output drops immediately and the elapsed time is reset.
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct OnDelay {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub delay: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
    pending_since_ms: u64,
}

impl Block for OnDelay {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        let input = matches!(self.input.get_value(), Some(Value::Bool(b)) if b.value);
        let delay_ms = input_to_millis_or_default(&self.delay.val);
        let now = current_time_millis();

        if !input {
            self.pending_since_ms = 0;
            self.out.set(Bool { value: false }.into());
            return;
        }

        if self.pending_since_ms == 0 {
            self.pending_since_ms = now;
        }

        let elapsed = now.saturating_sub(self.pending_since_ms);
        let on = elapsed >= delay_ms;
        self.out.set(Bool { value: on }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::timers::OnDelay,
    };

    #[tokio::test]
    async fn test_on_delay_zero_delay_passes_through() {
        let mut block = OnDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.delay, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_on_delay_input_false_is_off() {
        let mut block = OnDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.delay, (1000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_on_delay_long_delay_not_yet_elapsed() {
        let mut block = OnDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // 1 hour delay — won't have elapsed in test
        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.delay, (3_600_000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_on_delay_resets_when_input_drops() {
        let mut block = OnDelay::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Start the delay
        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.delay, (3_600_000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
        assert!(block.pending_since_ms > 0);

        // Input drops, timer resets
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.delay, (3_600_000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
        assert_eq!(block.pending_since_ms, 0);
    }
}
