// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::future::select_all;
use libhaystack::val::kind::HaystackKind;

use crate::base::{block::Block, input::input_reader::InputReader};

impl<B: Block> InputReader for B {
    async fn read_inputs(&mut self) -> Option<usize> {
        read_block_inputs(self).await
    }

    async fn wait_on_inputs(&mut self) {
        read_block_inputs_ignore_result(self).await;
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

pub(crate) async fn read_block_inputs_ignore_result<B: Block>(block: &mut B) {
    read_block_inputs(block).await;
}
