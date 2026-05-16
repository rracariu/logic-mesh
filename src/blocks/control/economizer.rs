// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_matching};

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Air-side economizer decision with hysteresis.
///
/// Compares an outdoor measurement (`oa`) against a return measurement
/// (`ra`). Use temperatures for a dry-bulb economizer or enthalpies for
/// an enthalpy economizer — the block doesn't care, as long as `oa`,
/// `ra`, `highLimit`, and `deadband` share a unit.
///
/// Output `enable` is a [Schmitt](super::Deadband)-style decision:
/// - **enable** when `oa < ra − deadband` AND `oa < highLimit`
/// - **disable** when `oa > ra` OR `oa > highLimit`
///   (the high-limit trip skips the deadband for safety)
/// - **hold** previous state otherwise
///
/// `highLimit` defaults to +∞ (no limit). `deadband` defaults to 0.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Economizer {
    #[input(kind = "Number")]
    pub oa: InputImpl,
    #[input(kind = "Number")]
    pub ra: InputImpl,
    #[input(name = "highLimit", kind = "Number")]
    pub high_limit: InputImpl,
    #[input(kind = "Number")]
    pub deadband: InputImpl,
    #[output(name = "enable", kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Economizer {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let oa_n = match input_as_number(&self.oa) {
            Some(n) => n,
            None => return,
        };
        let oa = oa_n.value;
        let ra = match input_as_number_matching(&self.ra, oa_n.unit) {
            Some(v) => v,
            None => return,
        };
        let high_limit =
            input_as_number_matching(&self.high_limit, oa_n.unit).unwrap_or(f64::INFINITY);
        let deadband = input_as_number_matching(&self.deadband, oa_n.unit)
            .unwrap_or(0.0)
            .abs();

        let current = matches!(&self.out.value, Value::Bool(b) if b.value);

        let next = if oa > ra || oa > high_limit {
            false
        } else if oa < ra - deadband && oa < high_limit {
            true
        } else {
            current
        };

        self.out.set(Bool { value: next }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::control::Economizer,
    };

    async fn run(oa: f64, ra: f64, high: f64, deadband: f64) -> bool {
        let mut block = Economizer::new();
        write_block_inputs([
            (&mut block.oa, oa),
            (&mut block.ra, ra),
            (&mut block.high_limit, high),
            (&mut block.deadband, deadband),
        ])
        .await;
        block.execute().await;
        matches!(block.out.value, libhaystack::val::Value::Bool(b) if b.value)
    }

    #[tokio::test]
    async fn test_economizer_enables_when_oa_cooler() {
        // OA 12°C, RA 24°C, well below high-limit 25°C, deadband 1°C
        assert!(run(12.0, 24.0, 25.0, 1.0).await);
    }

    #[tokio::test]
    async fn test_economizer_disables_above_high_limit() {
        // OA 28°C exceeds high-limit 25°C even though < RA 30°C
        assert!(!run(28.0, 30.0, 25.0, 1.0).await);
    }

    #[tokio::test]
    async fn test_economizer_disables_when_oa_warmer() {
        assert!(!run(26.0, 24.0, 30.0, 1.0).await);
    }

    #[tokio::test]
    async fn test_economizer_holds_in_deadband() {
        let mut block = Economizer::new();
        // Drive on with a cool OA
        write_block_inputs([
            (&mut block.oa, 12.0),
            (&mut block.ra, 24.0),
            (&mut block.high_limit, 30.0),
            (&mut block.deadband, 2.0),
        ])
        .await;
        block.execute().await;
        assert!(matches!(block.out.value, libhaystack::val::Value::Bool(b) if b.value));

        // OA rises into the deadband window (between RA-2 and RA): hold ON
        write_block_inputs([(&mut block.oa, 23.0)]).await;
        block.execute().await;
        assert!(matches!(block.out.value, libhaystack::val::Value::Bool(b) if b.value));

        // Cross above RA: trip OFF
        write_block_inputs([(&mut block.oa, 24.5)]).await;
        block.execute().await;
        assert!(matches!(block.out.value, libhaystack::val::Value::Bool(b) if !b.value));

        // Drop back into the deadband: hold OFF
        write_block_inputs([(&mut block.oa, 23.0)]).await;
        block.execute().await;
        assert!(matches!(block.out.value, libhaystack::val::Value::Bool(b) if !b.value));
    }
}
