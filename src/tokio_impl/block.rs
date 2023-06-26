// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use anyhow::Result;
use futures::future::select_all;
use futures::FutureExt;
use libhaystack::encoding::zinc;
use libhaystack::val::kind::HaystackKind;
use libhaystack::val::{Bool, Number, Str, Value};

use crate::base::{block::Block, input::input_reader::InputReader};
use crate::blocks::utils::sleep_millis;

impl<B: Block> InputReader for B {
    async fn read_inputs(&mut self) -> Option<usize> {
        read_block_inputs(self).await
    }

    async fn wait_on_inputs(&mut self, timeout: Duration) {
        let millis = timeout.as_millis() as u64;
        let (_, index, _) = select_all([
            sleep_millis(millis).boxed_local(),
            async {
                self.read_inputs().await;
            }
            .boxed_local(),
        ])
        .await;

        if index != 0 {
            sleep_millis(millis).await;
        }
    }
}

///
/// Reads all inputs and awaits for any of them to have data
/// On the first input that has data, read the data and update
/// the input's value.
///
/// If the input kind does not match the received Value kind, this would put the block in fault.
///
/// # Returns
/// The index of the input that was read with a valid value.
///
pub(crate) async fn read_block_inputs<B: Block>(block: &mut B) -> Option<usize> {
    let mut inputs = block
        .inputs_mut()
        .into_iter()
        .filter(|input| input.is_connected())
        .collect::<Vec<_>>();

    if inputs.is_empty() {
        return None;
    }

    let (val, idx, _) = {
        let input_futures = inputs
            .iter_mut()
            .map(|input| input.receiver())
            .collect::<Vec<_>>();

        select_all(input_futures).await
    };

    if let Some(value) = val {
        if let Some(input) = inputs.get_mut(idx) {
            let expected = *input.kind();
            let actual = HaystackKind::from(&value);

            if expected != actual {
                match convert_value(value, expected, actual) {
                    Ok(value) => input.set_value(value),
                    Err(err) => {
                        log::error!("Error converting value: {}", err);
                        block.set_state(crate::base::block::BlockState::Fault);
                    }
                }
            } else {
                input.set_value(value);
            }
        } else {
            block.set_state(crate::base::block::BlockState::Fault);
        }
        Some(idx)
    } else {
        None
    }
}

/// Converts a value from one kind to another.
///
/// # Arguments
///	- `val` - The value to convert
/// - `expected` - The expected kind of the value
/// - `actual` - The actual kind of the value
///
/// # Returns
/// The converted value if the conversion was successful.
fn convert_value(val: Value, expected: HaystackKind, actual: HaystackKind) -> Result<Value> {
    match (expected, actual) {
        (HaystackKind::Bool, HaystackKind::Bool) => Ok(val),
        (HaystackKind::Bool, HaystackKind::Number) => {
            let val = Number::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            Ok((val.value != 0.0).into())
        }
        (HaystackKind::Bool, HaystackKind::Str) => {
            let val = Str::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            let num = zinc::decode::from_str(&val.value)?;
            if num.is_bool() {
                Ok(num)
            } else {
                Err(anyhow::anyhow!("Expected a bool value, but got {:?}", val))
            }
        }

        (HaystackKind::Number, HaystackKind::Number) => Ok(val),
        (HaystackKind::Number, HaystackKind::Bool) => {
            let val = Bool::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            Ok((if val.value { 1 } else { 0 }).into())
        }
        (HaystackKind::Number, HaystackKind::Str) => {
            let val = Str::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            let num = zinc::decode::from_str(&val.value)?;
            if num.is_number() {
                Ok(num)
            } else {
                Err(anyhow::anyhow!(
                    "Expected a number value, but got {:?}",
                    val
                ))
            }
        }

        (HaystackKind::Str, HaystackKind::Str) => Ok(val),
        (HaystackKind::Str, HaystackKind::Bool) => Ok(val.to_string().as_str().into()),
        (HaystackKind::Str, HaystackKind::Number) => {
            let str = zinc::encode::to_zinc_string(&val)?;
            Ok(str.as_str().into())
        }
        _ => Err(anyhow::anyhow!(
            "Cannot convert {:?} to {:?}",
            actual,
            expected
        )),
    }
}
