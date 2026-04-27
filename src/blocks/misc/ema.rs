// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_to_millis_or_default};

use libhaystack::units::Unit;
use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Exponential moving average (first-order low-pass) filter.
/// `out[n] = alpha * in + (1 - alpha) * out[n-1]`
/// `alpha` is clamped to `[0..1]`. A larger alpha tracks the input more
/// closely; a smaller alpha smooths more heavily. The block samples at
/// `interval` milliseconds; on the first sample the output is seeded
/// with the current input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct Ema {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub alpha: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    last: f64,
    initialized: bool,
    unit: Option<&'static Unit>,
}

impl Block for Ema {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.interval.val);
        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let in_n = match input_as_number(&self.input) {
            Some(n) => n,
            None => return,
        };
        let input = in_n.value;
        self.unit = in_n.unit;
        let alpha = input_as_number(&self.alpha)
            .map(|n| n.value)
            .unwrap_or(0.1)
            .clamp(0.0, 1.0);

        let next = if !self.initialized {
            self.initialized = true;
            input
        } else {
            alpha * input + (1.0 - alpha) * self.last
        };

        self.last = next;
        self.out.set(match self.unit {
            Some(u) => Number::make_with_unit(next, u).into(),
            None => next.into(),
        });
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::misc::ema::Ema,
    };

    #[tokio::test]
    async fn test_ema_initial_seeds_to_input() {
        let mut block = Ema::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.alpha, (0.5).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (50.0).into());
    }

    #[tokio::test]
    async fn test_ema_smooths_step() {
        let mut block = Ema::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Seed at 0
        for _ in write_block_inputs(&mut [
            (&mut block.input, (0.0).into()),
            (&mut block.alpha, (0.5).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Step to 100; with alpha=0.5 the first post-seed sample lands at 50
        for _ in write_block_inputs(&mut [
            (&mut block.input, (100.0).into()),
            (&mut block.alpha, (0.5).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        if let Value::Number(n) = block.out.value {
            assert!((n.value - 50.0).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number output");
        }
    }

    #[tokio::test]
    async fn test_ema_alpha_one_passes_through() {
        let mut block = Ema::new();
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));

        // Seed at 0
        for _ in write_block_inputs(&mut [
            (&mut block.input, (0.0).into()),
            (&mut block.alpha, (1.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // alpha=1 means no smoothing
        for _ in write_block_inputs(&mut [
            (&mut block.input, (42.0).into()),
            (&mut block.alpha, (1.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (42.0).into());
    }
}
