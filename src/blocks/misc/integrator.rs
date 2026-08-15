// Copyright (c) 2022-2026, Radu Racariu.

//! Integrator block.

use std::time::Duration;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::Block,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_to_millis_or_default};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::val::Value;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Time integrator (totalizer). Accumulates `in * dt`, where `in` is in
/// units-per-second and `dt` is the elapsed wall-clock interval in
/// seconds. A rising edge on `reset` clears the accumulator. Useful for
/// totalizing energy (kWh from kW), volume (gallons from gpm/60), etc.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct Integrator {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Bool")]
    pub reset: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    accumulator: f64,
    last_time_ms: u64,
    prev_reset: Option<bool>,
}

impl Block for Integrator {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.interval.val);
        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let v = input_as_number(&self.input).map(|n| n.value).unwrap_or(0.0);
        let reset = matches!(self.reset.get_value(), Some(Value::Bool(b)) if b.value);
        let now = current_time_millis();

        let rising_reset = matches!(self.prev_reset, Some(false)) && reset;
        self.prev_reset = Some(reset);

        if rising_reset {
            self.accumulator = 0.0;
            self.last_time_ms = now;
            self.out.set(0.into());
            return;
        }

        if self.last_time_ms == 0 {
            self.last_time_ms = now;
            self.out.set(self.accumulator.into());
            return;
        }

        let dt_s = now.saturating_sub(self.last_time_ms) as f64 / 1000.0;
        self.accumulator += v * dt_s;
        self.last_time_ms = now;
        self.out.set(self.accumulator.into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, link::BaseLink},
        blocks::misc::integrator::Integrator,
    };

    fn link_out(block: &mut Integrator) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_integrator_starts_at_zero() {
        let mut block = Integrator::new();
        link_out(&mut block);

        write_block_inputs([
            (&mut block.input, 1.into()),
            (&mut block.reset, Value::make_false()),
            (&mut block.interval, 0.into()),
        ])
        .await;
        block.execute().await;
        assert_eq!(block.out.value, 0.into());
    }

    #[tokio::test]
    async fn test_integrator_accumulates() {
        let mut block = Integrator::new();
        link_out(&mut block);

        // First sample seeds last_time
        write_block_inputs([
            (&mut block.input, 10.into()),
            (&mut block.reset, Value::make_false()),
            (&mut block.interval, 0.into()),
        ])
        .await;
        block.execute().await;

        // Backdate last_time by 2s and re-execute
        block.last_time_ms = block.last_time_ms.saturating_sub(2000);
        write_block_inputs([(&mut block.input, 10)]).await;
        block.execute().await;

        if let Value::Number(n) = block.out.value {
            // ~ 10 * 2s = 20, allow some slop
            assert!(n.value >= 19.0 && n.value <= 22.0, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }

    #[tokio::test]
    async fn test_integrator_reset() {
        let mut block = Integrator::new();
        link_out(&mut block);
        block.accumulator = 100.0;

        write_block_inputs([
            (&mut block.input, Value::from(0)),
            (&mut block.reset, Value::from(false)),
            (&mut block.interval, Value::from(0)),
        ])
        .await;
        block.execute().await;

        write_block_inputs([(&mut block.reset, true)]).await;
        block.execute().await;
        assert_eq!(block.accumulator, 0.0);
    }
}
