// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_matching};

use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Change-of-value (COV) gate. The output mirrors `in`, but only updates
/// when the input has changed by at least `delta` from the last value
/// that was emitted. Useful for throttling chatty sensors and matching
/// BACnet COV semantics.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct ChangeOfValue {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub delta: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    last_emitted: Option<f64>,
}

impl Block for ChangeOfValue {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let in_n = match input_as_number(&self.input) {
            Some(n) => n,
            None => return,
        };
        let v = in_n.value;
        let delta = input_as_number_matching(&self.delta, in_n.unit).unwrap_or(0.0);

        let should_emit = match self.last_emitted {
            None => true,
            Some(prev) => (v - prev).abs() >= delta,
        };

        if should_emit {
            self.last_emitted = Some(v);
            self.out.set(match in_n.unit {
                Some(u) => Number::make_with_unit(v, u).into(),
                None => v.into(),
            });
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::misc::ChangeOfValue,
    };

    #[tokio::test]
    async fn test_cov_first_emit() {
        let mut block = ChangeOfValue::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.delta, (1.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (50.0).into());
    }

    #[tokio::test]
    async fn test_cov_skips_small_change() {
        let mut block = ChangeOfValue::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.delta, (1.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Change less than delta → output should remain
        for _ in write_block_inputs(&mut [(&mut block.input, (50.5).into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (50.0).into());
    }

    #[tokio::test]
    async fn test_cov_emits_on_large_change() {
        let mut block = ChangeOfValue::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input, (50.0).into()),
            (&mut block.delta, (1.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        for _ in write_block_inputs(&mut [(&mut block.input, (52.0).into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (52.0).into());
    }
}
