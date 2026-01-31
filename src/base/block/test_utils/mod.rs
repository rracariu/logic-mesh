use std::ops::Range;

use libhaystack::val::Value;

use crate::{
    base::input::Input,
    tokio_impl::{ReaderImpl, WriterImpl},
};

#[cfg(test)]
pub mod mock;

/// Writes the given values to the given inputs
/// and returns the range of indices of the inputs that were written to.
pub(crate) async fn write_block_inputs<'a>(
    values: &'a mut [(
        &'a mut dyn Input<Reader = ReaderImpl, Writer = WriterImpl>,
        Value,
    )],
) -> Range<u32> {
    for (input, value) in values.iter_mut() {
        if !input.is_connected() {
            input.increment_conn();
        }

        input.writer().try_send(value.clone()).unwrap();
    }

    0..(values.len() - 1) as u32
}
