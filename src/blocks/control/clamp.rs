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

/// Clamp the input to the inclusive range `[min..max]`. If `min > max`
/// the bounds are swapped so the block is still well-defined.
///
/// `min` and `max` are read in the same unit as `in`; bounds with a
/// compatible unit are converted, bounds without a unit are taken
/// as-is. The output carries the unit of `in`.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Clamp {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(kind = "Number")]
    pub min: InputImpl,
    #[input(kind = "Number")]
    pub max: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Clamp {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let n = match input_as_number(&self.input) {
            Some(n) => n,
            None => return,
        };
        let lo = match input_as_number_matching(&self.min, n.unit) {
            Some(v) => v,
            None => return,
        };
        let hi = match input_as_number_matching(&self.max, n.unit) {
            Some(v) => v,
            None => return,
        };
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };

        let value = n.value.clamp(lo, hi);
        self.out.set(match n.unit {
            Some(u) => Number::make_with_unit(value, u).into(),
            None => value.into(),
        });
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        base::input::input_reader::InputReader, blocks::control::Clamp,
    };

    async fn run(input: f64, min: f64, max: f64) -> f64 {
        let mut block = Clamp::new();
        for _ in write_block_inputs(&mut [
            (&mut block.input, input.into()),
            (&mut block.min, min.into()),
            (&mut block.max, max.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => n.value,
            _ => panic!("expected number"),
        }
    }

    #[tokio::test]
    async fn test_clamp_within() {
        assert_eq!(run(5.0, 0.0, 10.0).await, 5.0);
    }

    #[tokio::test]
    async fn test_clamp_high() {
        assert_eq!(run(15.0, 0.0, 10.0).await, 10.0);
    }

    #[tokio::test]
    async fn test_clamp_low() {
        assert_eq!(run(-5.0, 0.0, 10.0).await, 0.0);
    }

    #[tokio::test]
    async fn test_clamp_swapped_bounds() {
        // min > max → bounds get swapped so the block stays well-defined
        assert_eq!(run(5.0, 10.0, 0.0).await, 5.0);
    }
}
