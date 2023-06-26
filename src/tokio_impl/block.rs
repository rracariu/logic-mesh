// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::time::Duration;

use futures::future::select_all;
use futures::FutureExt;
use libhaystack::val::kind::HaystackKind;

use crate::base::{block::Block, input::input_reader::InputReader};
use crate::blocks::utils::sleep_millis;

impl<B: Block> InputReader for B {
    async fn read_inputs(&mut self) -> Option<usize> {
        read_block_inputs(self).await
    }

    async fn wait_on_inputs(&mut self, timeout: Duration) {
        let millis = timeout.as_millis() as u64;
        let _ = select_all([
            sleep_millis(millis).boxed_local(),
            (async move || {
                if self.read_inputs().await.is_none() {
                    sleep_millis(millis).await;
                };
            })()
            .boxed_local(),
        ])
        .await;
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
