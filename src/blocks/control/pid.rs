// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::blocks::utils::input_as_number;
use crate::blocks::utils::input_to_millis_or_default;
use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps, input_reader::InputReader},
        output::Output,
    },
    blocks::utils::input_as_float_or_default,
};
use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Discrete PID controller.
///
/// `error = sp − pv`, output = `P + I + D`, clamped to `[min..max]`.
///
/// - **P**: `Kp · error`
/// - **I** (trapezoidal, `dt` in seconds): `I += Ki · dt/2 · (error + error_prev)`,
///   clamped to `[min..max]` for simple anti-windup so a saturated actuator
///   does not wind the integral past the achievable output.
/// - **D** (filtered derivative on measurement): acting on the PV (not the
///   error) prevents derivative kick on setpoint changes.
///   `D = −(bias·Kd·(pv − pv_prev) + (bias − dt)·D_prev) / (bias + dt)`
///
/// `bias` is the derivative-filter time constant in seconds (default 0.1 s,
/// i.e. ~100 ms filter). Keep `bias ≥ dt` for stable filter behavior.
///
/// `interval` accepts any time unit (`ms`, `s`, `min`, `h`); defaults to 200 ms.
///
/// If `input` is not connected, the controller's previous output is used as
/// the process variable — convenient for demos that should converge to SP
/// without an explicit plant model.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Pid {
    #[input(kind = "Number")]
    pub input: InputImpl,

    #[input(kind = "Number")]
    pub sp: InputImpl,

    #[input(kind = "Number")]
    pub kp: InputImpl,

    #[input(kind = "Number")]
    pub ki: InputImpl,

    #[input(kind = "Number")]
    pub kd: InputImpl,

    #[input(kind = "Number")]
    pub interval: InputImpl,

    #[input(kind = "Number")]
    pub min: InputImpl,

    #[input(kind = "Number")]
    pub max: InputImpl,

    #[input(kind = "Number")]
    pub bias: InputImpl,

    #[output(kind = "Number")]
    pub out: OutputImpl,

    integral: f64,
    derivative: f64,
    last_error: f64,
    last_pv: f64,
    initialized: bool,
}

impl Block for Pid {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.interval.val);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let kp = input_as_number(&self.kp).map(|v| v.value).unwrap_or(0.98);
        let ki = input_as_number(&self.ki).map(|v| v.value).unwrap_or(0.002);
        let kd = input_as_number(&self.kd).map(|v| v.value).unwrap_or(0.25);
        let bias = input_as_number(&self.bias).map(|v| v.value).unwrap_or(0.1);

        // Negative gains are nonsense; zero is fine (yields a P/PI/PD/I-only loop).
        if kp < 0.0 || ki < 0.0 || kd < 0.0 {
            return;
        }

        let sp = input_as_float_or_default(&self.sp);
        let min = input_as_number(&self.min).map(|v| v.value).unwrap_or(0.0);
        let max = input_as_number(&self.max).map(|v| v.value).unwrap_or(100.0);
        let dt = millis as f64 / 1000.0;
        let (clamp_lo, clamp_hi) = if min <= max { (min, max) } else { (max, min) };

        let cur_value = if self.input.is_connected() {
            input_as_number(&self.input).map(|v| v.value).unwrap_or(0.0)
        } else {
            TryInto::<f64>::try_into(self.out.value()).unwrap_or(0.0)
        };

        // Seed history on the first cycle so the first integral step uses
        // a real (rather than zero) `error_prev` and the derivative does
        // not see a fake step from `last_pv = 0`.
        let error = sp - cur_value;
        if !self.initialized {
            self.last_pv = cur_value;
            self.last_error = error;
            self.initialized = true;
        }

        let proportional = kp * error;

        // Trapezoidal integration with anti-windup clamp.
        self.integral += ki * dt * 0.5 * (error + self.last_error);
        self.integral = self.integral.clamp(clamp_lo, clamp_hi);

        // Filtered derivative on measurement.
        self.derivative =
            -(bias * kd * (cur_value - self.last_pv) + (bias - dt) * self.derivative) / (bias + dt);

        let output = (proportional + self.integral + self.derivative).clamp(clamp_lo, clamp_hi);

        self.last_pv = cur_value;
        self.last_error = error;

        self.out.set(output.into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, link::BaseLink},
        blocks::control::Pid,
    };

    fn link_out(block: &mut Pid) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    fn out_value(block: &Pid) -> f64 {
        match block.out.value {
            Value::Number(n) => n.value,
            _ => panic!("expected Number output, got {:?}", block.out.value),
        }
    }

    /// P-only with `input` unconnected uses the previous output as the PV
    /// (self-feedback). The closed-loop fixed point is `v = Kp·SP / (1+Kp)`.
    /// With Kp = 0.5 and SP = 50 that's ~16.67, with a permanent offset of ~33.
    #[tokio::test]
    async fn test_pid_p_only_steady_state_offset() {
        let mut block = Pid::new();
        link_out(&mut block);

        write_block_inputs([
            (&mut block.interval, 1.0),
            (&mut block.sp, 50.0),
            (&mut block.kp, 0.5),
            (&mut block.ki, 0.0),
            (&mut block.kd, 0.0),
        ])
        .await;

        for _ in 0..100 {
            block.execute().await;
        }

        let v = out_value(&block);
        let expected = 0.5 * 50.0 / 1.5; // ≈ 16.667
        assert!(
            (v - expected).abs() < 0.5,
            "P-only should settle at {} ± 0.5, got {}",
            expected,
            v
        );
    }

    /// PI with the same self-feedback loop should drive the integral until
    /// the steady-state error is zero and the output equals the setpoint.
    #[tokio::test]
    async fn test_pid_pi_drives_error_to_zero() {
        let mut block = Pid::new();
        link_out(&mut block);

        write_block_inputs([
            (&mut block.interval, 1.0),
            (&mut block.sp, 50.0),
            (&mut block.kp, 0.3),
            (&mut block.ki, 100.0),
            (&mut block.kd, 0.0),
        ])
        .await;

        for _ in 0..200 {
            block.execute().await;
        }

        let v = out_value(&block);
        assert!(
            (v - 50.0).abs() < 1.0,
            "PI should converge to SP within 1.0, got {}",
            v
        );
    }

    /// All gains zero is a valid configuration (the controller is a no-op).
    /// This guards against the previous `gain <= 0 → return` regression that
    /// silently disabled P-only and PI controllers.
    #[tokio::test]
    async fn test_pid_zero_gains_outputs_zero() {
        let mut block = Pid::new();
        link_out(&mut block);

        write_block_inputs([
            (&mut block.interval, 1.0),
            (&mut block.sp, 50.0),
            (&mut block.kp, 0.0),
            (&mut block.ki, 0.0),
            (&mut block.kd, 0.0),
        ])
        .await;

        for _ in 0..10 {
            block.execute().await;
        }

        assert_eq!(out_value(&block), 0.0);
    }
}
