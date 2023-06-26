use std::ops::Range;

use libhaystack::val::Value;

use crate::base::input::props::InputProps;
use crate::blocks::InputImpl;

#[cfg(test)]
pub mod mock;

/// Writes the given values to the given inputs
/// and returns the range of indices of the inputs that were written to.
pub(crate) async fn write_block_inputs<'a>(
    values: &'a mut [(&'a mut InputImpl, Value)],
) -> Range<u32> {
    for (input, value) in values.iter_mut() {
        if input.connection_count == 0 {
            input.increment_conn();
        }

        input.writer().try_send(value.clone()).unwrap();
    }

    0..(values.len() - 1) as u32
}
