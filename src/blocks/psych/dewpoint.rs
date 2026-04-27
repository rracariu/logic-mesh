// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_in};

use libhaystack::units::units_generated::CELSIUS;
use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Dew-point temperature from dry-bulb temperature and relative humidity.
/// Uses the Magnus formula with Alduchov-Eskridge coefficients
/// (a = 17.625, b = 243.04 °C), accurate to about 0.4 °C over the
/// building range.
///
/// `t` accepts any temperature unit (°C, °F, K) and is converted to °C
/// internally. The output is always tagged °C.
#[block]
#[derive(BlockProps, Debug)]
#[category = "psych"]
pub struct Dewpoint {
    #[input(name = "t", kind = "Number")]
    pub temperature: InputImpl,
    #[input(name = "rh", kind = "Number")]
    pub humidity: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Dewpoint {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let t = match input_as_number_in(&self.temperature, &CELSIUS) {
            Some(v) => v,
            None => return,
        };
        let rh = match input_as_number(&self.humidity) {
            // ln(0) is undefined — clamp away from absolute zero RH
            Some(n) => n.value.clamp(0.001, 100.0),
            None => return,
        };

        const A: f64 = 17.625;
        const B: f64 = 243.04;
        let alpha = (rh / 100.0).ln() + A * t / (B + t);
        let td = B * alpha / (A - alpha);
        self.out.set(Number::make_with_unit(td, &CELSIUS).into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::{
        units::units_generated::{CELSIUS, FAHRENHEIT},
        val::{Number, Value},
    };

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::psych::Dewpoint,
    };

    fn approx(actual: f64, expected: f64, tol: f64) {
        assert!(
            (actual - expected).abs() < tol,
            "expected ~{}, got {}",
            expected,
            actual
        );
    }

    async fn run(t: f64, rh: f64) -> f64 {
        let mut block = Dewpoint::new();
        for _ in write_block_inputs(&mut [
            (&mut block.temperature, t.into()),
            (&mut block.humidity, rh.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => n.value,
            _ => panic!("expected Number"),
        }
    }

    #[tokio::test]
    async fn test_dewpoint_saturated() {
        // 100% RH → dewpoint == dry-bulb
        approx(run(20.0, 100.0).await, 20.0, 0.5);
    }

    #[tokio::test]
    async fn test_dewpoint_typical_room() {
        // 24°C, 50% RH ≈ 12.9°C dewpoint
        approx(run(24.0, 50.0).await, 12.9, 0.5);
    }

    #[tokio::test]
    async fn test_dewpoint_below_dry_bulb() {
        // Dewpoint must always be ≤ dry-bulb for RH ≤ 100%.
        let t = 30.0;
        for rh in [10.0_f64, 30.0, 60.0, 90.0] {
            let td = run(t, rh).await;
            assert!(td <= t + 0.01, "rh={}, td={}", rh, td);
        }
    }

    #[tokio::test]
    async fn test_dewpoint_accepts_fahrenheit_input() {
        // 75°F = 23.89°C, 50% RH ≈ 12.8°C dewpoint
        let mut block = Dewpoint::new();
        for _ in write_block_inputs(&mut [
            (
                &mut block.temperature,
                Number::make_with_unit(75.0, &FAHRENHEIT).into(),
            ),
            (&mut block.humidity, (50.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => {
                assert_eq!(n.unit, Some(&CELSIUS));
                approx(n.value, 12.8, 0.5);
            }
            _ => panic!("expected Number"),
        }
    }
}
