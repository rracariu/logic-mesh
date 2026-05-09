use libhaystack::val::Value;

use crate::{
    base::input::Input,
    tokio_impl::{ReaderImpl, WriterImpl},
};

#[cfg(test)]
pub mod mock;

/// Writes the given values to the given inputs as if they had arrived from
/// connected upstream sources. Each input is marked connected (so the block
/// will treat it as a real input) and the value is pushed through the
/// underlying watch channel — a subsequent `block.read_inputs().await` (or
/// the first `wait_on_inputs` inside the block's `execute()`) will drain
/// every input in one pass, populating their `.val` fields.
pub(crate) async fn write_block_inputs<'a, V, const N: usize>(
    values: [(
        &'a mut dyn Input<Reader = ReaderImpl, Writer = WriterImpl>,
        V,
    ); N],
) where
    V: Into<Value>,
{
    for (input, value) in values {
        if !input.is_connected() {
            input.increment_conn();
        }
        input.writer().send(value.into()).unwrap();
    }
}
