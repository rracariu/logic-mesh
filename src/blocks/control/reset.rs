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

/// Linear reset (range scale). Maps `in` from `[inMin..inMax]` onto
/// `[outMin..outMax]` and clamps the result to that output range.
/// Inverted output ranges (`outMax < outMin`) are supported, which is
/// the common BAS reset pattern (e.g. SAT falls as OAT rises).
///
/// `inMin` / `inMax` are matched to the unit of `in`; `outMin` /
/// `outMax` define the output range and dictate the output's unit.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Reset {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[input(name = "inMin", kind = "Number")]
    pub in_min: InputImpl,
    #[input(name = "inMax", kind = "Number")]
    pub in_max: InputImpl,
    #[input(name = "outMin", kind = "Number")]
    pub out_min: InputImpl,
    #[input(name = "outMax", kind = "Number")]
    pub out_max: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Reset {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let in_n = match input_as_number(&self.input) {
            Some(n) => n,
            None => return,
        };
        let in_min = match input_as_number_matching(&self.in_min, in_n.unit) {
            Some(v) => v,
            None => return,
        };
        let in_max = match input_as_number_matching(&self.in_max, in_n.unit) {
            Some(v) => v,
            None => return,
        };
        let out_min_n = match input_as_number(&self.out_min) {
            Some(n) => n,
            None => return,
        };
        let out_unit = out_min_n
            .unit
            .or(input_as_number(&self.out_max).and_then(|n| n.unit));
        let out_min = out_min_n.value;
        let out_max = match input_as_number_matching(&self.out_max, out_unit) {
            Some(v) => v,
            None => return,
        };

        let span = in_max - in_min;
        let result = if span == 0.0 {
            out_min
        } else {
            let t = (in_n.value - in_min) / span;
            out_min + t * (out_max - out_min)
        };

        let (lo, hi) = if out_min <= out_max {
            (out_min, out_max)
        } else {
            (out_max, out_min)
        };
        let clamped = result.clamp(lo, hi);

        self.out.set(match out_unit {
            Some(u) => Number::make_with_unit(clamped, u).into(),
            None => clamped.into(),
        });
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        base::input::input_reader::InputReader, blocks::control::Reset,
    };
    use libhaystack::val::Value;

    async fn run(input: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
        let mut block = Reset::new();
        for _ in write_block_inputs(&mut [
            (&mut block.input, input.into()),
            (&mut block.in_min, in_min.into()),
            (&mut block.in_max, in_max.into()),
            (&mut block.out_min, out_min.into()),
            (&mut block.out_max, out_max.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => n.value,
            _ => panic!("expected Number output"),
        }
    }

    #[tokio::test]
    async fn test_reset_midpoint() {
        let v = run(50.0, 0.0, 100.0, 0.0, 10.0).await;
        assert!((v - 5.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_reset_clamp_high() {
        let v = run(150.0, 0.0, 100.0, 0.0, 10.0).await;
        assert_eq!(v, 10.0);
    }

    #[tokio::test]
    async fn test_reset_clamp_low() {
        let v = run(-10.0, 0.0, 100.0, 0.0, 10.0).await;
        assert_eq!(v, 0.0);
    }

    #[tokio::test]
    async fn test_reset_inverted_output_range() {
        // SAT reset: as OAT rises 50 -> 70, SAT falls 65 -> 55
        let v = run(60.0, 50.0, 70.0, 65.0, 55.0).await;
        assert!((v - 60.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_reset_inverted_clamp() {
        let v = run(80.0, 50.0, 70.0, 65.0, 55.0).await;
        assert_eq!(v, 55.0);
    }
}
