// Copyright (c) 2022-2026, Radu Racariu.

use std::collections::VecDeque;
use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_to_millis_or_default};

use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Simple moving average over the last N samples. The block samples at
/// `interval` milliseconds and reports the unweighted mean of the most
/// recent `window` samples (clamped to at least 1).
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct MovingAverage {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub window: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    samples: VecDeque<f64>,
}

impl Block for MovingAverage {
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
        let v = in_n.value;

        let window = input_as_number(&self.window)
            .map(|n| n.value as usize)
            .unwrap_or(10)
            .max(1);

        self.samples.push_back(v);
        while self.samples.len() > window {
            self.samples.pop_front();
        }

        let sum: f64 = self.samples.iter().sum();
        let mean = sum / self.samples.len() as f64;
        self.out.set(match in_n.unit {
            Some(u) => Number::make_with_unit(mean, u).into(),
            None => mean.into(),
        });
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, link::BaseLink},
        blocks::misc::moving_average::MovingAverage,
    };

    fn link_out(block: &mut MovingAverage) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_moving_average_single_sample() {
        let mut block = MovingAverage::new();
        link_out(&mut block);

        write_block_inputs([
            (&mut block.input, 10),
            (&mut block.window, 3),
            (&mut block.interval, 0),
        ])
        .await;
        block.execute().await;
        assert_eq!(block.out.value, (10.0).into());
    }

    #[tokio::test]
    async fn test_moving_average_three_samples() {
        let mut block = MovingAverage::new();
        link_out(&mut block);

        for v in [10.0_f64, 20.0, 30.0] {
            write_block_inputs([
                (&mut block.input, v),
                (&mut block.window, 3.0),
                (&mut block.interval, 0.0),
            ])
            .await;
            block.execute().await;
        }

        if let Value::Number(n) = block.out.value {
            assert!((n.value - 20.0).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }

    #[tokio::test]
    async fn test_moving_average_window_drop() {
        let mut block = MovingAverage::new();
        link_out(&mut block);

        // Window of 2 — only the last two samples count
        for v in [10.0_f64, 20.0, 30.0, 40.0] {
            write_block_inputs([
                (&mut block.input, v),
                (&mut block.window, 2.0),
                (&mut block.interval, 0.0),
            ])
            .await;
            block.execute().await;
        }

        if let Value::Number(n) = block.out.value {
            // mean(30, 40) = 35
            assert!((n.value - 35.0).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }
}
