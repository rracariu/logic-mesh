// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_in};

use libhaystack::units::units_generated::{CELSIUS, KILOJOULES_PER_KILOGRAM_DRY_AIR, PASCAL};
use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Moist-air specific enthalpy in kJ per kg of dry air. The dry-bulb
/// temperature accepts any temperature unit (°C, °F, K) and is converted
/// to °C internally; pressure accepts Pa or kPa and is converted to Pa
/// (default 101 325, sea-level standard). Relative humidity is in %.
///
/// Saturation pressure uses the Tetens approximation; this is accurate
/// to better than 0.1% over the typical building range and is what most
/// BAS controllers use for economizer enthalpy comparisons.
#[block]
#[derive(BlockProps, Debug)]
#[category = "psych"]
pub struct Enthalpy {
    #[input(name = "t", kind = "Number")]
    pub temperature: InputImpl,
    #[input(name = "rh", kind = "Number")]
    pub humidity: InputImpl,
    #[input(name = "p", kind = "Number")]
    pub pressure: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Enthalpy {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let t = match input_as_number_in(&self.temperature, &CELSIUS) {
            Some(v) => v,
            None => return,
        };
        let rh = match input_as_number(&self.humidity) {
            Some(n) => n.value.clamp(0.0, 100.0),
            None => return,
        };
        let p = input_as_number_in(&self.pressure, &PASCAL)
            .filter(|v| *v > 0.0)
            .unwrap_or(101_325.0);

        // Tetens: saturation vapor pressure (Pa) over liquid water for T in °C
        let ps = 610.78 * (17.27 * t / (t + 237.3)).exp();
        let pw = (rh / 100.0) * ps;
        let denom = p - pw;
        // Avoid blowing up at saturation in extreme/invalid inputs.
        let w = if denom > 0.0 { 0.622 * pw / denom } else { 0.0 };

        // ASHRAE moist-air enthalpy (kJ/kg dry air), T in °C
        let h = 1.006 * t + w * (2501.0 + 1.86 * t);
        self.out
            .set(Number::make_with_unit(h, &KILOJOULES_PER_KILOGRAM_DRY_AIR).into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::{
        units::units_generated::{FAHRENHEIT, KILOJOULES_PER_KILOGRAM_DRY_AIR, KILOPASCAL},
        val::{Number, Value},
    };

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::psych::Enthalpy,
    };

    fn approx(actual: f64, expected: f64, tol: f64) {
        assert!(
            (actual - expected).abs() < tol,
            "expected ~{}, got {}",
            expected,
            actual
        );
    }

    async fn run(t: f64, rh: f64, p: f64) -> f64 {
        let mut block = Enthalpy::new();
        for _ in write_block_inputs(&mut [
            (&mut block.temperature, t.into()),
            (&mut block.humidity, rh.into()),
            (&mut block.pressure, p.into()),
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
    async fn test_enthalpy_dry_air_zero_celsius() {
        // 0°C, 0% RH → h = 0 kJ/kg
        approx(run(0.0, 0.0, 101_325.0).await, 0.0, 0.01);
    }

    #[tokio::test]
    async fn test_enthalpy_room_conditions() {
        // 24°C, 50% RH at sea level — typical occupied space, ~47.6 kJ/kg
        approx(run(24.0, 50.0, 101_325.0).await, 47.6, 1.0);
    }

    #[tokio::test]
    async fn test_enthalpy_higher_rh_higher_h() {
        let dry = run(25.0, 20.0, 101_325.0).await;
        let humid = run(25.0, 80.0, 101_325.0).await;
        assert!(humid > dry);
    }

    #[tokio::test]
    async fn test_enthalpy_accepts_fahrenheit_and_kpa() {
        // 75°F (≈ 23.89°C), 50% RH, 101.325 kPa → ≈ 47.4 kJ/kg
        let mut block = Enthalpy::new();
        for _ in write_block_inputs(&mut [
            (
                &mut block.temperature,
                Number::make_with_unit(75.0, &FAHRENHEIT).into(),
            ),
            (&mut block.humidity, (50.0).into()),
            (
                &mut block.pressure,
                Number::make_with_unit(101.325, &KILOPASCAL).into(),
            ),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        match block.out.value {
            Value::Number(n) => {
                assert_eq!(n.unit, Some(&KILOJOULES_PER_KILOGRAM_DRY_AIR));
                approx(n.value, 47.4, 1.0);
            }
            _ => panic!("expected Number"),
        }
    }
}
