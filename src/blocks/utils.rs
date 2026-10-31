// Copyright (c) 2022-2023, Radu Racariu.

//! Utility functions used by blocks.

#![allow(dead_code)]

use super::InputImpl;
use crate::base::error::{Result, ValueError};
use libhaystack::{
    units::{Unit, units_generated::MILLISECOND},
    val::{Number, Value},
};

/// The duration, in milliseconds, a timer input that holds no valid
/// duration stands for.
const DEFAULT_SLEEP_DUR: u64 = 200;

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

/// Reads a numeric input and returns its value in `target` units.
///
/// - If the input has no unit set, the raw value is returned (lenient).
/// - If the input's unit equals `target`, the raw value is returned.
/// - If the input's unit is convertible to `target`, the converted value is returned.
/// - If the input is missing or has an incompatible unit, returns [`None`].
pub(super) fn input_as_number_in(input: &InputImpl, target: &'static Unit) -> Option<f64> {
    let n = input_as_number(input)?;
    match n.unit {
        None => Some(n.value),
        Some(u) if u == target => Some(n.value),
        Some(u) => u.convert_to(n.value, target).ok(),
    }
}

/// Reads a numeric input, optionally converting to `target` if a target
/// unit is known. Used by unit-preserving blocks where the target unit
/// is dictated by another input (typically `in`).
pub(super) fn input_as_number_matching(
    input: &InputImpl,
    target: Option<&'static Unit>,
) -> Option<f64> {
    match target {
        Some(u) => input_as_number_in(input, u),
        None => input_as_number(input).map(|n| n.value),
    }
}

/// Converts the duration to milliseconds, or returns `DEFAULT_SLEEP_DUR`
/// if the conversion fails.
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

/// The unit a set of numbers converts to: the first one any of them
/// has.
pub(super) fn common_unit(numbers: impl IntoIterator<Item = Number>) -> Option<&'static Unit> {
    numbers.into_iter().find_map(|n| n.unit)
}

/// `number` in `unit`: converted from another unit, given `unit` if it
/// has none, and unchanged when `unit` is [`None`].
///
/// # Errors
///
/// Returns an error if unit conversion fails.
pub(super) fn to_unit(number: Number, unit: Option<&'static Unit>) -> Result<Number, ValueError> {
    match (number.unit, unit) {
        (Some(from), Some(to)) if from != to => from
            .convert_to(number.value, to)
            .map_err(ValueError::UnitConversion)
            .map(|value| Number {
                value,
                unit: Some(to),
            }),
        (None, Some(to)) => Ok(Number {
            value: number.value,
            unit: Some(to),
        }),
        _ => Ok(number),
    }
}

/// Converts all `numbers` to a common unit, in place.
///
/// # Errors
///
/// Returns an error if unit conversion fails.
pub(super) fn convert_units(numbers: &mut [Number]) -> Result<(), ValueError> {
    let unit = common_unit(numbers.iter().copied());
    for number in numbers {
        *number = to_unit(*number, unit)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use libhaystack::units::units_generated::{CELSIUS, FAHRENHEIT, METER};
    use libhaystack::val::Number;

    use super::convert_units;

    #[test]
    fn numbers_convert_to_the_first_unit_any_of_them_has() {
        let mut numbers = [
            Number::make(1.0),
            Number::make_with_unit(212.0, &FAHRENHEIT),
            Number::make_with_unit(20.0, &CELSIUS),
        ];

        convert_units(&mut numbers).expect("temperatures convert");

        assert_eq!(
            numbers.map(|n| (n.value.round(), n.unit)),
            [
                (1.0, Some(&FAHRENHEIT)),
                (212.0, Some(&FAHRENHEIT)),
                (68.0, Some(&FAHRENHEIT)),
            ]
        );
    }

    #[test]
    fn numbers_without_units_are_left_as_they_are() {
        let mut numbers = [Number::make(1.0), Number::make(2.0)];
        convert_units(&mut numbers).expect("nothing to convert");
        assert_eq!(numbers, [Number::make(1.0), Number::make(2.0)]);
    }

    #[test]
    fn incompatible_units_do_not_convert() {
        let mut numbers = [
            Number::make_with_unit(1.0, &CELSIUS),
            Number::make_with_unit(1.0, &METER),
        ];
        assert!(convert_units(&mut numbers).is_err());
    }
}
