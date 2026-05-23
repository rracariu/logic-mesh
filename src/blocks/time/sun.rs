// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{input_as_number, input_as_number_in};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::units::units_generated::MINUTE;
use libhaystack::val::{Bool, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Sunrise / sunset times for a given location, plus a `isDay` indicator.
///
/// Inputs:
/// - `lat` — latitude in degrees, north positive (-90..90)
/// - `lon` — longitude in degrees, east positive (-180..180)
/// - `tzOffset` — minutes from UTC; accepts any time unit (`-5h`, `60min`).
///
/// Outputs (minutes since local midnight, 0..1440):
/// - `sunrise` — local time of sunrise
/// - `sunset` — local time of sunset
/// - `isDay` — true when the local clock is between sunrise and sunset.
///   At polar latitudes where the sun never rises or never sets, `isDay`
///   reflects the polar state (true on polar day, false on polar night)
///   and `sunrise` / `sunset` retain their previous values.
///
/// Uses the Wikipedia "Sunrise equation" — a simplified NOAA formulation
/// accurate to within a few minutes for typical latitudes.
///
/// ## Timing
///
/// The first execute emits as soon as the inputs are available (no
/// startup wait). Subsequent executes align to the next wall-clock
/// minute boundary, so multiple `Sun` blocks (or repeated program
/// loads of the same program) stay phase-aligned. Editing `lat`,
/// `lon`, or `tzOffset` wakes the block immediately for a fresh
/// compute — `wait_on_inputs` returns early on any input change.
///
/// Trade-off vs. the previous 1-second polling: `isDay` transitions
/// can now lag up to ~60 s behind the actual sunrise/sunset wall-
/// clock time. The underlying sunrise equation is itself only
/// accurate to ±a few minutes, so this is well within the model's
/// own error bars and fine for lighting / scheduling use cases.
#[block]
#[derive(BlockProps, Debug)]
#[category = "time"]
pub struct Sun {
    #[input(kind = "Number")]
    pub lat: InputImpl,
    #[input(kind = "Number")]
    pub lon: InputImpl,
    #[input(name = "tzOffset", kind = "Number")]
    pub tz_offset: InputImpl,
    #[output(kind = "Number")]
    pub sunrise: OutputImpl,
    #[output(kind = "Number")]
    pub sunset: OutputImpl,
    #[output(name = "isDay", kind = "Bool")]
    pub is_day: OutputImpl,
    /// First-execute marker. Cleared via `BlockState::Default` (false)
    /// when the block is constructed; set after a successful emit so
    /// subsequent cycles use the minute-boundary cadence.
    initialized: bool,
}

impl Block for Sun {
    async fn execute(&mut self) {
        // Cadence:
        // - First execute: skip the wait entirely, compute now.
        // - Subsequent executes: wait until the next wall-clock
        //   minute boundary. `wait_on_inputs` also returns early on
        //   any input change, so coord/tz edits get an immediate
        //   recompute.
        if self.initialized {
            let now_ms = current_time_millis() as i64;
            let until_next_minute_ms = (60_000 - now_ms.rem_euclid(60_000)) as u64;
            self.wait_on_inputs(Duration::from_millis(until_next_minute_ms))
                .await;
        }

        let lat = match input_as_number(&self.lat) {
            Some(n) => n.value,
            None => {
                // Inputs not yet populated — typically only seen if
                // execute fires before `load_program` has issued its
                // WriteInput commands. A brief back-off avoids
                // tight-looping while we wait for the constants to
                // land. The next cycle will retry.
                self.wait_on_inputs(Duration::from_millis(250)).await;
                return;
            }
        };
        let lon = match input_as_number(&self.lon) {
            Some(n) => n.value,
            None => {
                self.wait_on_inputs(Duration::from_millis(250)).await;
                return;
            }
        };
        let tz_offset_min = input_as_number_in(&self.tz_offset, &MINUTE)
            .map(|v| v as i64)
            .unwrap_or(0);

        let now_ms = current_time_millis() as i64;
        let local_secs = (now_ms + tz_offset_min * 60_000).div_euclid(1000);
        let now_minutes = (local_secs.rem_euclid(86_400) / 60) as f64;

        match compute_sun_times(now_ms, lat, lon, tz_offset_min) {
            SunState::Normal {
                rise_minutes,
                set_minutes,
            } => {
                self.sunrise.set(rise_minutes.into());
                self.sunset.set(set_minutes.into());

                let is_day = if rise_minutes <= set_minutes {
                    now_minutes >= rise_minutes && now_minutes < set_minutes
                } else {
                    // Window crosses local midnight (rare — extreme tz/lon mix)
                    now_minutes >= rise_minutes || now_minutes < set_minutes
                };
                self.is_day.set(Bool { value: is_day }.into());
            }
            SunState::PolarDay => {
                self.is_day.set(Bool { value: true }.into());
            }
            SunState::PolarNight => {
                self.is_day.set(Bool { value: false }.into());
            }
        }

        // Mark initialized only after a successful emit so the very
        // first compute can't be skipped to the next minute boundary.
        self.initialized = true;
    }
}

#[derive(Debug, PartialEq)]
enum SunState {
    Normal { rise_minutes: f64, set_minutes: f64 },
    PolarDay,
    PolarNight,
}

/// Days from 1970-01-01 00:00 UTC to 2000-01-01 00:00 UTC. Used to convert
/// between unix-day counts and the J2000-relative day count `n` from the
/// sunrise equation.
const DAYS_1970_TO_2000: i64 = 10_957;

/// `n` is the integer count of days since J2000.0 (2000-01-01 12:00 UTC);
/// the +0.0008 is the leap-second correction from the Wikipedia algorithm.
fn compute_sun_times(now_utc_ms: i64, lat_deg: f64, lon_deg: f64, tz_offset_min: i64) -> SunState {
    let local_secs = (now_utc_ms + tz_offset_min * 60_000).div_euclid(1000);
    let local_days = local_secs.div_euclid(86_400);

    let n = (local_days - DAYS_1970_TO_2000) as f64 + 0.0008;
    let j_star = n - lon_deg / 360.0;

    let m_deg = (357.5291 + 0.985_600_28 * j_star).rem_euclid(360.0);
    let m_rad = m_deg.to_radians();

    let c_deg = 1.9148 * m_rad.sin() + 0.0200 * (2.0 * m_rad).sin() + 0.0003 * (3.0 * m_rad).sin();

    let lambda_rad = (m_deg + c_deg + 180.0 + 102.9372)
        .rem_euclid(360.0)
        .to_radians();

    let j_transit = j_star + 0.0053 * m_rad.sin() - 0.0069 * (2.0 * lambda_rad).sin();

    let sin_decl = lambda_rad.sin() * 23.44_f64.to_radians().sin();
    let cos_decl = (1.0 - sin_decl * sin_decl).sqrt();

    let lat_rad = lat_deg.to_radians();
    let cos_omega =
        ((-0.83_f64).to_radians().sin() - lat_rad.sin() * sin_decl) / (lat_rad.cos() * cos_decl);

    if cos_omega > 1.0 {
        return SunState::PolarNight;
    }
    if cos_omega < -1.0 {
        return SunState::PolarDay;
    }

    let omega_frac = cos_omega.acos() / std::f64::consts::TAU;
    let j_rise_n = j_transit - omega_frac;
    let j_set_n = j_transit + omega_frac;

    let to_minutes = |jn: f64| -> f64 {
        // jn is days since J2000 12:00 UTC. Add 10957.5 to express as
        // unix-days (since 1970-01-01 00:00 UTC), shift by tzOffset, then
        // take fraction-of-day → minutes-since-local-midnight.
        let unix_days = jn + 10_957.5;
        let local_days_frac = unix_days + tz_offset_min as f64 / 1440.0;
        local_days_frac.rem_euclid(1.0) * 1440.0
    };

    SunState::Normal {
        rise_minutes: to_minutes(j_rise_n),
        set_minutes: to_minutes(j_set_n),
    }
}

#[cfg(test)]
mod test {
    use super::{SunState, compute_sun_times};

    /// 2024-03-20 00:00 UTC (vernal equinox) in unix milliseconds.
    /// 19_802 days since 1970-01-01 × 86_400_000 ms/day.
    const EQUINOX_2024_MS: i64 = 19_802 * 86_400_000;

    /// 2024-06-20 00:00 UTC (summer solstice). 19_894 days since epoch.
    const SOLSTICE_2024_MS: i64 = 19_894 * 86_400_000;

    fn assert_close(actual: f64, expected: f64, tol_minutes: f64, label: &str) {
        assert!(
            (actual - expected).abs() <= tol_minutes,
            "{}: expected {} ± {}, got {}",
            label,
            expected,
            tol_minutes,
            actual
        );
    }

    #[test]
    fn test_sun_equator_equinox_is_about_six_to_six() {
        // Equator + equinox should give sunrise ~06:00 and sunset ~18:00 (± a
        // few minutes for atmospheric refraction).
        let s = compute_sun_times(EQUINOX_2024_MS + 12 * 3_600_000, 0.0, 0.0, 0);
        match s {
            SunState::Normal {
                rise_minutes,
                set_minutes,
            } => {
                assert_close(rise_minutes, 6.0 * 60.0, 15.0, "equator equinox sunrise");
                assert_close(set_minutes, 18.0 * 60.0, 15.0, "equator equinox sunset");
            }
            _ => panic!("expected Normal, got {:?}", s),
        }
    }

    #[test]
    fn test_sun_nyc_solstice() {
        // NYC, summer solstice 2024: sunrise ≈ 05:25 EDT, sunset ≈ 20:31 EDT.
        let s = compute_sun_times(SOLSTICE_2024_MS + 12 * 3_600_000, 40.7128, -74.0060, -240);
        match s {
            SunState::Normal {
                rise_minutes,
                set_minutes,
            } => {
                assert_close(
                    rise_minutes,
                    5.0 * 60.0 + 25.0,
                    20.0,
                    "NYC solstice sunrise",
                );
                assert_close(set_minutes, 20.0 * 60.0 + 31.0, 20.0, "NYC solstice sunset");
            }
            _ => panic!("expected Normal, got {:?}", s),
        }
    }

    #[test]
    fn test_sun_polar_day_in_arctic_summer() {
        // 80°N at summer solstice — sun never sets.
        let s = compute_sun_times(SOLSTICE_2024_MS + 12 * 3_600_000, 80.0, 0.0, 0);
        assert_eq!(s, SunState::PolarDay);
    }

    #[test]
    fn test_sun_polar_night_in_arctic_winter() {
        // 80°N at winter solstice (2024-12-21 ≈ unix day 20078).
        let winter_ms = 20_078_i64 * 86_400_000 + 12 * 3_600_000;
        let s = compute_sun_times(winter_ms, 80.0, 0.0, 0);
        assert_eq!(s, SunState::PolarNight);
    }

    #[test]
    fn test_sun_day_length_decreases_toward_winter_in_north() {
        // At 50°N, day length should be shorter on Dec 21 than Jun 21.
        let summer = compute_sun_times(SOLSTICE_2024_MS + 12 * 3_600_000, 50.0, 0.0, 0);
        let winter_ms = 20_078_i64 * 86_400_000 + 12 * 3_600_000;
        let winter = compute_sun_times(winter_ms, 50.0, 0.0, 0);

        let summer_len = match summer {
            SunState::Normal {
                rise_minutes,
                set_minutes,
            } => set_minutes - rise_minutes,
            _ => panic!("expected Normal in summer"),
        };
        let winter_len = match winter {
            SunState::Normal {
                rise_minutes,
                set_minutes,
            } => set_minutes - rise_minutes,
            _ => panic!("expected Normal in winter"),
        };
        assert!(
            summer_len > winter_len,
            "summer day ({} min) should be longer than winter ({} min)",
            summer_len,
            winter_len
        );
    }
}
