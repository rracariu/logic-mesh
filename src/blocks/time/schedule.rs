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

/// Weekly time-of-day schedule. Output is true when the local clock falls
/// inside the `[start..end)` window on one of the selected `days`.
///
/// - `start` / `end`: time of day in `"HH:MM"` (24h). When `start > end`
///   the window is treated as crossing midnight (e.g. 22:00 → 06:00).
/// - `days`: a string of day-letters using the BACnet/scheduling convention
///   `M T W R F S U` (R = Thursday, U = Sunday). For example weekdays =
///   `"MTWRF"`, weekends = `"SU"`, every day = `"MTWRFSU"`.
/// - `tzOffset`: time to add to UTC to reach the local zone. Accepts any
///   time unit (e.g. `-5h` for EST, `60min` for CET, `300s`). A bare
///   number is interpreted as minutes for backwards compatibility.
///   Defaults to 0.
///
/// Output is the boolean `occupied`. The block re-evaluates once per second.
#[block]
#[derive(BlockProps, Debug)]
#[category = "time"]
pub struct Schedule {
    #[input(kind = "Str")]
    pub start: InputImpl,
    #[input(kind = "Str")]
    pub end: InputImpl,
    #[input(kind = "Str")]
    pub days: InputImpl,
    #[input(name = "tzOffset", kind = "Number")]
    pub tz_offset: InputImpl,
    #[output(name = "occupied", kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for Schedule {
    async fn execute(&mut self) {
        // Schedules change at minute boundaries; polling once per second is plenty.
        self.wait_on_inputs(Duration::from_millis(1000)).await;

        if !self.out.is_connected() {
            return;
        }

        let start = match str_input(&self.start).and_then(parse_hhmm) {
            Some(v) => v,
            None => {
                self.out.set(Bool { value: false }.into());
                return;
            }
        };
        let end = match str_input(&self.end).and_then(parse_hhmm) {
            Some(v) => v,
            None => {
                self.out.set(Bool { value: false }.into());
                return;
            }
        };
        let days_mask = match str_input(&self.days).map(parse_days) {
            Some(m) => m,
            None => {
                self.out.set(Bool { value: false }.into());
                return;
            }
        };

        let tz_offset_min = input_as_number_in(&self.tz_offset, &MINUTE)
            .map(|v| v as i64)
            .unwrap_or(0);

        let utc_ms = current_time_millis() as i64;
        let local_ms = utc_ms + tz_offset_min * 60_000;
        // floor-division to days/seconds to handle negative tz offsets correctly
        let local_secs = local_ms.div_euclid(1000);
        let days_since_epoch = local_secs.div_euclid(86_400);
        let seconds_of_day = local_secs.rem_euclid(86_400) as u32;

        // 1970-01-01 was Thursday; map so Monday = 0 .. Sunday = 6.
        // (days + 3) % 7: Thu→3, Fri→4, Sat→5, Sun→6, Mon→0, Tue→1, Wed→2.
        let dow = ((days_since_epoch + 3).rem_euclid(7)) as u8;

        let now_minutes = seconds_of_day / 60;

        // A schedule window is associated with the day it *opens on*, so the
        // post-midnight tail (00:00..end) belongs to the previous day's mask bit.
        let occupied = if start == end {
            false
        } else if start < end {
            (now_minutes >= start && now_minutes < end) && (days_mask & (1 << dow)) != 0
        } else if now_minutes >= start {
            (days_mask & (1 << dow)) != 0
        } else if now_minutes < end {
            let prev = (dow + 6) % 7;
            (days_mask & (1 << prev)) != 0
        } else {
            false
        };

        self.out.set(Bool { value: occupied }.into());
    }
}

fn str_input(input: &InputImpl) -> Option<&str> {
    match &input.val {
        Some(Value::Str(s)) => Some(s.value.as_str()),
        _ => None,
    }
}

/// Parse "HH:MM" (24h) to minutes-since-midnight. Returns `None` on bad input.
fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let h: u32 = h.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// Parse a day-mask string like "MTWRF" into a bitmask where bit 0 = Monday
/// .. bit 6 = Sunday. Unknown characters are ignored.
fn parse_days(s: &str) -> u8 {
    let mut mask = 0u8;
    for c in s.chars() {
        let bit = match c.to_ascii_uppercase() {
            'M' => 0,
            'T' => 1,
            'W' => 2,
            'R' => 3,
            'F' => 4,
            'S' => 5,
            'U' => 6,
            _ => continue,
        };
        mask |= 1 << bit;
    }
    mask
}

#[cfg(test)]
mod test {
    use super::{parse_days, parse_hhmm};

    #[test]
    fn test_parse_hhmm_ok() {
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("08:30"), Some(8 * 60 + 30));
        assert_eq!(parse_hhmm("23:59"), Some(23 * 60 + 59));
    }

    #[test]
    fn test_parse_hhmm_invalid() {
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("nope"), None);
        assert_eq!(parse_hhmm(""), None);
    }

    #[test]
    fn test_parse_days_weekdays() {
        let m = parse_days("MTWRF");
        assert_eq!(m, 0b0011111);
    }

    #[test]
    fn test_parse_days_full_week() {
        let m = parse_days("MTWRFSU");
        assert_eq!(m, 0b1111111);
    }

    #[test]
    fn test_parse_days_weekend() {
        let m = parse_days("SU");
        assert_eq!(m, 0b1100000);
    }

    #[test]
    fn test_parse_days_ignores_unknown() {
        assert_eq!(parse_days("M, T, W"), parse_days("MTW"));
    }
}
