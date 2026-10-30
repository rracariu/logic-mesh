// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::block::BlockState;
use crate::base::input::InputProps;
use crate::base::output::Output;
use crate::blocks::{InputImpl, OutputImpl};
use libhaystack::val::Value;

/// Sets `out` to `calc` of the two inputs as integers, and returns the
/// state the block is in: a fault while either input is not a number.
pub(super) fn execute_impl(
    in1: &InputImpl,
    in2: &InputImpl,
    out: &mut OutputImpl,
    calc: impl FnOnce(i64, i64) -> i64,
) -> BlockState {
    if let (Some(Value::Number(in1)), Some(Value::Number(in2))) = (in1.value(), in2.value()) {
        out.set(Value::make_int(calc(in1.value as i64, in2.value as i64)));
        BlockState::Running
    } else {
        BlockState::fault("bitwise: non-numeric input")
    }
}
