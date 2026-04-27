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
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Discrete derivative `d(in)/dt`, output in units of `in` per second.
/// Sampled at `interval` milliseconds. The first sample seeds the
/// previous value so the first output is zero.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct Derivative {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    last_value: f64,
    last_time_ms: u64,
    initialized: bool,
}

impl Block for Derivative {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.interval.val);
        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let v = match input_as_number(&self.input) {
            Some(n) => n.value,
            None => return,
        };
        let now = current_time_millis();

        if !self.initialized {
            self.initialized = true;
            self.last_value = v;
            self.last_time_ms = now;
            self.out.set((0.0_f64).into());
            return;
        }

        let dt_ms = now.saturating_sub(self.last_time_ms);
        let rate = if dt_ms == 0 {
            0.0
        } else {
            (v - self.last_value) * 1000.0 / dt_ms as f64
        };

        self.last_value = v;
        self.last_time_ms = now;
        self.out.set(rate.into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::misc::derivative::Derivative,
    };

    fn link_out(block: &mut Derivative) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_derivative_first_sample_zero() {
        let mut block = Derivative::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (0.0_f64).into());
    }

    #[tokio::test]
    async fn test_derivative_change() {
        let mut block = Derivative::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.input, (10.0).into()),
            (&mut block.interval, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Force a known time delta
        block.last_time_ms = block.last_time_ms.saturating_sub(1000);

        for _ in write_block_inputs(&mut [(&mut block.input, (20.0).into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;

        // ~10 units / 1s — allow slop for the 1s elapsed during execute
        if let Value::Number(n) = block.out.value {
            assert!(n.value > 0.0 && n.value <= 10.0, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }
}
