// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_in};

use libhaystack::units::units_generated::CELSIUS;
use libhaystack::val::{Number, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Wet-bulb temperature from dry-bulb temperature and relative humidity.
///
/// Uses Stull's empirical correlation (Stull 2011). No iteration needed.
/// Accurate to ~0.4 °C across the validity range:
/// `T ∈ [-20, 50] °C`, `RH ∈ [5, 99] %`, `P ≈ 101.325 kPa` (sea level).
/// Outside that envelope the value is still finite but degrades; the
/// block does not error.
///
/// `t` accepts any temperature unit (°C, °F, K) and is converted to °C
/// internally; the output is tagged °C.
#[block]
#[derive(BlockProps, Debug)]
#[category = "psych"]
pub struct WetBulb {
    #[input(name = "t", kind = "Number")]
    pub temperature: InputImpl,
    #[input(name = "rh", kind = "Number")]
    pub humidity: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for WetBulb {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let t = match input_as_number_in(&self.temperature, &CELSIUS) {
            Some(v) => v,
            None => return,
        };
        let rh = match input_as_number(&self.humidity) {
            Some(n) => n.value.clamp(0.001, 100.0),
            None => return,
        };

        let tw = stull(t, rh);
        self.out.set(Number::make_with_unit(tw, &CELSIUS).into());
    }
}

fn stull(t_c: f64, rh: f64) -> f64 {
    t_c * (0.151_977 * (rh + 8.313_659).sqrt()).atan() + (t_c + rh).atan() - (rh - 1.676_331).atan()
        + 0.003_918_38 * rh.powf(1.5) * (0.023_101 * rh).atan()
        - 4.686_035
}

#[cfg(test)]
mod test {
    use super::stull;
    use libhaystack::{
        units::units_generated::{CELSIUS, FAHRENHEIT},
        val::{Number, Value},
    };

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::psych::WetBulb,
    };

    fn approx(actual: f64, expected: f64, tol: f64) {
        assert!(
            (actual - expected).abs() < tol,
            "expected ~{}, got {}",
            expected,
            actual
        );
    }

    #[test]
    fn test_stull_saturated_air() {
        // 100% RH → wet-bulb ≈ dry-bulb
        approx(stull(20.0, 100.0), 20.0, 0.5);
    }

    #[test]
    fn test_stull_typical_room() {
        // 24 °C / 50 % RH ≈ 17.0 °C wet-bulb
        approx(stull(24.0, 50.0), 17.0, 0.5);
    }

    #[test]
    fn test_stull_below_dry_bulb() {
        // For RH < 100 % the wet-bulb must be strictly less than dry-bulb.
        let t = 30.0;
        for rh in [10.0_f64, 30.0, 60.0, 90.0] {
            let tw = stull(t, rh);
            assert!(tw < t, "rh={}, tw={}", rh, tw);
        }
    }

    #[tokio::test]
    async fn test_wet_bulb_block_emits_celsius() {
        let mut block = WetBulb::new();
        write_block_inputs([(&mut block.temperature, 24.0), (&mut block.humidity, 50.0)]).await;
        block.execute().await;
        match block.out.value {
            Value::Number(n) => {
                assert_eq!(n.unit, Some(&CELSIUS));
                approx(n.value, 17.0, 0.5);
            }
            _ => panic!("expected Number"),
        }
    }

    #[tokio::test]
    async fn test_wet_bulb_block_accepts_fahrenheit() {
        // 75 °F (≈ 23.89 °C) at 50 % RH → ~16.9 °C wet-bulb
        let mut block = WetBulb::new();
        write_block_inputs([
            (
                &mut block.temperature,
                Number::make_with_unit(75.0, &FAHRENHEIT),
            ),
            (&mut block.humidity, 50.into()),
        ])
        .await;
        block.execute().await;
        match block.out.value {
            Value::Number(n) => {
                assert_eq!(n.unit, Some(&CELSIUS));
                approx(n.value, 16.9, 0.7);
            }
            _ => panic!("expected Number"),
        }
    }
}
