// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::future::select_all;
use libhaystack::val::kind::HaystackKind;

use crate::base::block::Block;

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
    let input_futures = block
        .inputs_mut()
        .into_iter()
        .filter(|input| input.is_connected())
        .map(|input| input.receiver())
        .collect::<Vec<_>>();

    if input_futures.is_empty() {
        return None;
    }

    let (val, idx, _) = select_all(input_futures).await;

    if let Some(value) = val {
        let mut inputs = block.inputs_mut();
        if let Some(input) = inputs.get_mut(idx) {
            if *input.kind() != HaystackKind::from(&value) {
                block.set_state(crate::base::block::BlockState::Fault);
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
