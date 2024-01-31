// Copyright (c) 2022-2023, Radu Racariu.

use std::sync::atomic::AtomicU64;

use super::InputImpl;
use anyhow::Result;
use libhaystack::{
    units::units_generated::MILLISECOND,
    val::{Number, Value},
};

/// Default value for sleep intervals
pub(crate) const DEFAULT_SLEEP_DUR: u64 = 200;

/// A global variable that controls the sleep duration used
/// to schedule the execution of blocks.
pub static SLEEP_DUR: AtomicU64 = AtomicU64::new(DEFAULT_SLEEP_DUR);

pub(super) fn input_as_float_or_default(input: &InputImpl) -> f64 {
    input_as_number(input).map(|v| v.value).unwrap_or(0.0)
}

pub(super) fn input_as_number(input: &InputImpl) -> Option<Number> {
    if let Some(Value::Number(val)) = input.val {
        Some(val)
    } else {
        None
    }
}

/// Convert the duration to milliseconds, or return the default with
/// `DEFAULT_SLEEP_DUR` if the conversion fails.
pub(super) fn input_to_millis_or_default(dur: &Option<Value>) -> u64 {
    if let Some(Value::Number(dur)) = dur {
        if let Some(unit) = dur.unit {
            match unit.convert_to(dur.value, &MILLISECOND) {
                Ok(millis) => millis as u64,
                Err(_) => DEFAULT_SLEEP_DUR,
            }
        } else {
            dur.value as u64
        }
    } else {
        DEFAULT_SLEEP_DUR
    }
}

/// Convert all the numbers to the same unit
///
/// # Arguments
/// numbers - the numbers to convert
///
/// # Returns
/// A vector of numbers with the same unit
pub(super) fn convert_units(numbers: &[Number]) -> Result<Vec<Number>> {
    if numbers.len() <= 1 {
        Ok(numbers.to_vec())
    } else if let Some(unit) = numbers
        .iter()
        .find_map(|n| if n.unit.is_some() { n.unit } else { None })
    {
        numbers
            .iter()
            .map(|n| {
                if let Some(other_unit) = n.unit {
                    if other_unit != unit {
                        other_unit
                            .convert_to(n.value, unit)
                            .map_err(|err| anyhow::anyhow!(err))
                            .map(|v| Number {
                                value: v,
                                unit: Some(unit),
                            })
                    } else {
                        Ok(*n)
                    }
                } else {
                    Ok(Number {
                        value: n.value,
                        unit: Some(unit),
                    })
                }
            })
            .collect::<Result<Vec<Number>>>()
    } else {
        Ok(numbers.to_vec())
    }
}
