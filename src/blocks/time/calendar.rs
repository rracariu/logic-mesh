// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::input_as_number_in;
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::units::units_generated::MINUTE;
use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Date list match. The output is true when today (in the configured
/// time zone) appears in the comma-separated `dates` input.
///
/// `dates` accepts `"YYYY-MM-DD"` entries separated by commas, spaces,
/// or semicolons (mix freely). `tzOffset` accepts any time unit
/// (e.g. `-5h`, `60min`); a bare number is interpreted as minutes.
///
/// Compose with [`Schedule`] (e.g. `schedule AND NOT calendar`) to
/// implement holiday overrides.
#[block]
#[derive(BlockProps, Debug)]
#[category = "time"]
pub struct Calendar {
    #[input(kind = "Str")]
    pub dates: InputImpl,
    #[input(name = "tzOffset", kind = "Number")]
    pub tz_offset: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Calendar {
    async fn execute(&mut self) {
        // Day boundaries are coarse — re-evaluate roughly once per second.
        self.wait_on_inputs(Duration::from_millis(1000)).await;

        if !self.out.is_connected() {
            return;
        }

        let dates_str = match &self.dates.val {
            Some(Value::Str(s)) => s.value.as_str(),
            _ => {
                self.out.set(Bool { value: false }.into());
                return;
            }
        };

        let tz_offset_min = input_as_number_in(&self.tz_offset, &MINUTE)
            .map(|v| v as i64)
            .unwrap_or(0);

        let utc_ms = current_time_millis() as i64;
        let local_secs = (utc_ms + tz_offset_min * 60_000).div_euclid(1000);
        let days_since_epoch = local_secs.div_euclid(86_400);
        let (y, m, d) = civil_from_days(days_since_epoch);

        let today = format!("{:04}-{:02}-{:02}", y, m, d);
        let matched = dates_str
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .map(|s| s.trim())
            .any(|s| !s.is_empty() && s == today);

        self.out.set(Bool { value: matched }.into());
    }
}

/// Convert days-since-epoch (1970-01-01) to (year, month, day).
/// Implementation of Howard Hinnant's civil-from-days algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod test {
    use super::civil_from_days;

    #[test]
    fn test_civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn test_civil_from_days_known_dates() {
        // 2000-01-01 = 10957 days after epoch
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        // 2024-02-29 (leap day)
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        // 1969-12-31
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
