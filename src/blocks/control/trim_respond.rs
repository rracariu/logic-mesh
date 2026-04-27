// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_matching, input_to_millis_or_default};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::units::Unit;
use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// ASHRAE Guideline 36 trim-and-respond setpoint reset.
/// Once per `period` ms the setpoint is adjusted:
/// - if `requests <= ignore` → `sp += trim`
/// - else → `sp += respond * (requests - ignore)`, capped per cycle by
///   `maxChange` (when `maxChange > 0`)
///
/// Use signed values: e.g. for "increase SP to respond" (duct static
/// pressure) set `trim = -0.05`, `respond = +0.05`. The output is held
/// inside `[min..max]`.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct TrimRespond {
    #[input(kind = "Number")]
    pub requests: InputImpl,
    #[input(kind = "Number")]
    pub period: InputImpl,
    #[input(kind = "Number")]
    pub ignore: InputImpl,
    #[input(kind = "Number")]
    pub trim: InputImpl,
    #[input(kind = "Number")]
    pub respond: InputImpl,
    #[input(name = "maxChange", kind = "Number")]
    pub max_change: InputImpl,
    #[input(kind = "Number")]
    pub min: InputImpl,
    #[input(kind = "Number")]
    pub max: InputImpl,
    #[input(kind = "Number")]
    pub initial: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    sp: f64,
    last_update_ms: u64,
    initialized: bool,
    sp_unit: Option<&'static Unit>,
}

impl Block for TrimRespond {
    async fn execute(&mut self) {
        let period_ms = input_to_millis_or_default(&self.period.val);
        // Poll well below the period so we can fire near the boundary.
        let poll = period_ms.clamp(50, 1000);
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        // Setpoint scale unit: prefer initial's unit, else min, else max.
        let initial_n = input_as_number(&self.initial);
        let sp_unit = initial_n
            .and_then(|n| n.unit)
            .or_else(|| input_as_number(&self.min).and_then(|n| n.unit))
            .or_else(|| input_as_number(&self.max).and_then(|n| n.unit));

        let lo = input_as_number_matching(&self.min, sp_unit).unwrap_or(f64::NEG_INFINITY);
        let hi = input_as_number_matching(&self.max, sp_unit).unwrap_or(f64::INFINITY);
        let initial = input_as_number_matching(&self.initial, sp_unit)
            .unwrap_or_else(|| if lo.is_finite() { lo } else { 0.0 });

        let emit = |sp: f64| -> libhaystack::val::Value {
            match sp_unit {
                Some(u) => Number::make_with_unit(sp, u).into(),
                None => sp.into(),
            }
        };

        if !self.initialized {
            self.initialized = true;
            self.sp = initial.clamp(lo.min(hi), lo.max(hi));
            self.sp_unit = sp_unit;
            self.last_update_ms = current_time_millis();
            self.out.set(emit(self.sp));
            return;
        }

        let now = current_time_millis();
        let elapsed = now.saturating_sub(self.last_update_ms);
        if elapsed < period_ms {
            self.out.set(emit(self.sp));
            return;
        }
        self.last_update_ms = now;

        let requests = input_as_number(&self.requests)
            .map(|n| n.value)
            .unwrap_or(0.0);
        let ignore = input_as_number(&self.ignore)
            .map(|n| n.value)
            .unwrap_or(2.0);
        let trim = input_as_number_matching(&self.trim, sp_unit).unwrap_or(0.0);
        let respond = input_as_number_matching(&self.respond, sp_unit).unwrap_or(0.0);
        let max_change = input_as_number_matching(&self.max_change, sp_unit)
            .map(|v| v.abs())
            .unwrap_or(f64::INFINITY);

        let raw_change = if requests <= ignore {
            trim
        } else {
            respond * (requests - ignore)
        };

        let bounded = raw_change.clamp(-max_change, max_change);
        let (clamp_lo, clamp_hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        self.sp = (self.sp + bounded).clamp(clamp_lo, clamp_hi);
        self.sp_unit = sp_unit;
        self.out.set(emit(self.sp));
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::control::TrimRespond,
    };

    fn link_out(block: &mut TrimRespond) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive(
        block: &mut TrimRespond,
        requests: f64,
        ignore: f64,
        trim: f64,
        respond: f64,
        max_change: f64,
        min: f64,
        max: f64,
        initial: f64,
    ) {
        for _ in write_block_inputs(&mut [
            (&mut block.requests, requests.into()),
            (&mut block.period, (0).into()),
            (&mut block.ignore, ignore.into()),
            (&mut block.trim, trim.into()),
            (&mut block.respond, respond.into()),
            (&mut block.max_change, max_change.into()),
            (&mut block.min, min.into()),
            (&mut block.max, max.into()),
            (&mut block.initial, initial.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
    }

    #[tokio::test]
    async fn test_trim_respond_initial_sp() {
        let mut block = TrimRespond::new();
        link_out(&mut block);
        drive(&mut block, 0.0, 2.0, -0.05, 0.05, 0.5, 0.0, 2.0, 0.5).await;
        block.execute().await;
        assert_eq!(block.out.value, (0.5_f64).into());
    }

    #[tokio::test]
    async fn test_trim_respond_responds_to_requests() {
        let mut block = TrimRespond::new();
        link_out(&mut block);

        // Initialize at 0.5
        drive(&mut block, 0.0, 2.0, -0.05, 0.05, 0.5, 0.0, 2.0, 0.5).await;
        block.execute().await;

        // 5 requests, ignore 2 → 3 excess * 0.05 = 0.15
        drive(&mut block, 5.0, 2.0, -0.05, 0.05, 0.5, 0.0, 2.0, 0.5).await;
        block.execute().await;
        if let Value::Number(n) = block.out.value {
            assert!((n.value - 0.65).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }

    #[tokio::test]
    async fn test_trim_respond_trims_when_quiet() {
        let mut block = TrimRespond::new();
        link_out(&mut block);

        drive(&mut block, 0.0, 2.0, -0.05, 0.05, 0.5, 0.0, 2.0, 1.0).await;
        block.execute().await;
        // Initial 1.0, no requests, trim -0.05 → 0.95
        drive(&mut block, 0.0, 2.0, -0.05, 0.05, 0.5, 0.0, 2.0, 1.0).await;
        block.execute().await;
        if let Value::Number(n) = block.out.value {
            assert!((n.value - 0.95).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }

    #[tokio::test]
    async fn test_trim_respond_clamps_to_max_change() {
        let mut block = TrimRespond::new();
        link_out(&mut block);

        drive(&mut block, 0.0, 2.0, -0.05, 0.05, 0.1, 0.0, 5.0, 1.0).await;
        block.execute().await;
        // 100 requests * 0.05 = 5.0 raw; capped at maxChange=0.1
        drive(&mut block, 100.0, 0.0, -0.05, 0.05, 0.1, 0.0, 5.0, 1.0).await;
        block.execute().await;
        if let Value::Number(n) = block.out.value {
            assert!((n.value - 1.1).abs() < 1e-9, "got {}", n.value);
        } else {
            panic!("expected Number");
        }
    }
}
