// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_float_or_default, input_to_millis_or_default};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Rate Limit block. The output follows the input but is prevented from changing
/// faster than the specified maximum rate per second.
/// The `rising` input sets the max rate of increase per second,
/// and the `falling` input sets the max rate of decrease per second.
#[block]
#[derive(BlockProps, Debug)]
#[category = "timers"]
pub struct RateLimit {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub rising: InputImpl,
    #[input(kind = "Number")]
    pub falling: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    last_time_ms: u64,
}

impl Block for RateLimit {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if !self.out.is_connected() {
            return;
        }

        let now_ms = current_time_millis();
        let millis = input_to_millis_or_default(&self.interval.val);
        let target = input_as_float_or_default(&self.input);
        let rising_rate = input_as_float_or_default(&self.rising);
        let falling_rate = input_as_float_or_default(&self.falling);

        // First cycle: initialize output and record time
        if self.last_time_ms == 0 {
            self.last_time_ms = now_ms;
            self.out.set(target.into());
            return;
        }

        // Only update output at the rate specified by interval
        let elapsed_ms = now_ms.saturating_sub(self.last_time_ms);
        if elapsed_ms < millis {
            return;
        }

        let dt = elapsed_ms as f64 / 1000.0;
        self.last_time_ms = now_ms;

        let current = match self.out.value {
            Value::Number(n) => n.value,
            _ => 0.0,
        };

        let diff = target - current;

        let next = if diff > 0.0 && rising_rate > 0.0 {
            let max_rise = rising_rate * dt;
            current + diff.min(max_rise)
        } else if diff < 0.0 && falling_rate > 0.0 {
            let max_fall = falling_rate * dt;
            current - (-diff).min(max_fall)
        } else if diff > 0.0 && rising_rate <= 0.0 {
            target
        } else if diff < 0.0 && falling_rate <= 0.0 {
            target
        } else {
            current
        };

        self.out.set(next.into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::timers::RateLimit,
    };

    #[tokio::test]
    async fn test_rate_limit_initial() {
        let mut block = RateLimit::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        for _ in write_block_inputs(&mut [
            (&mut block.input, (100.0).into()),
            (&mut block.rising, (10.0).into()),
            (&mut block.falling, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        // First execution initializes output to input value
        block.execute().await;
        assert_eq!(block.out.value, (100.0).into());
    }

    #[tokio::test]
    async fn test_rate_limit_clamped() {
        let mut block = RateLimit::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Initialize at 0
        for _ in write_block_inputs(&mut [
            (&mut block.input, (0.0).into()),
            (&mut block.rising, (10.0).into()),
            (&mut block.falling, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (0.0).into());

        // Target jumps to 1000, but rate is 10/s and dt is near-zero
        // so output should barely move (be less than target)
        for _ in write_block_inputs(&mut [
            (&mut block.input, (1000.0).into()),
            (&mut block.rising, (10.0).into()),
            (&mut block.falling, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        if let Value::Number(n) = block.out.value {
            assert!(n.value < 1000.0, "output should be rate-limited");
            assert!(n.value >= 0.0, "output should not go negative");
        } else {
            panic!("expected Number output");
        }
    }

    #[tokio::test]
    async fn test_rate_limit_no_limit_passthrough() {
        let mut block = RateLimit::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Initialize at 0
        for _ in write_block_inputs(&mut [
            (&mut block.input, (0.0).into()),
            (&mut block.rising, (0.0).into()),
            (&mut block.falling, (0.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // With rate 0, should pass through directly regardless of dt
        for _ in write_block_inputs(&mut [
            (&mut block.input, (100.0).into()),
            (&mut block.rising, (0.0).into()),
            (&mut block.falling, (0.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (100.0).into());
    }

    #[tokio::test]
    async fn test_rate_limit_holds_at_target() {
        let mut block = RateLimit::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Initialize at 50
        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.rising, (10.0).into()),
            (&mut block.falling, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (50.0).into());

        // Same target, output should stay at 50
        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.rising, (10.0).into()),
            (&mut block.falling, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (50.0).into());
    }
}
