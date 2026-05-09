// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use futures::FutureExt;
use futures::future::select_all;
use libhaystack::val::kind::HaystackKind;

use super::sleep::sleep_millis;
use crate::base::block::{BlockState, convert_value_kind};
use crate::base::input::InputProps;
use crate::base::{block::Block, input::input_reader::InputReader};
use crate::blocks::InputImpl;
use crate::blocks::utils::get_sleep_dur;

pub type ReaderImpl = <InputImpl as InputProps>::Reader;
pub type WriterImpl = <InputImpl as InputProps>::Writer;

impl<B: Block> InputReader for B {
    async fn read_inputs(&mut self) -> Option<usize> {
        read_block_inputs(self).await
    }

    async fn read_inputs_until_ready(&mut self) -> Option<usize> {
        // Cap the polling cost when no input is reactive. `read_block_inputs`
        // returns `None` only when the block has no connected inputs at all
        // (e.g. during program load, before links are wired) or when every
        // connected upstream channel has been closed. In both cases there is
        // nothing to gain from polling at the per-cycle cadence — a new value
        // can only arrive via an engine event that's orders of magnitude
        // slower than a tight loop. So we back off exponentially from the
        // configured sleep duration up to a 2 s cap, then reset on the next
        // call once data flows again.
        const MAX_BACKOFF_MS: u64 = 2000;
        let mut backoff = get_sleep_dur();
        loop {
            let result = read_block_inputs(self).await;
            if result.is_some() {
                return result;
            }
            sleep_millis(backoff).await;
            backoff = (backoff.saturating_mul(2)).min(MAX_BACKOFF_MS);
        }
    }

    async fn wait_on_inputs(&mut self, timeout: Duration) -> Option<usize> {
        // Wait up to `timeout`, returning early only when an input *actually*
        // arrives. If `read_inputs` resolves immediately with `None` (no
        // connected inputs, or every connected channel was closed), let the
        // sleep branch be the throttle — otherwise periodic blocks would
        // tight-loop and starve the executor.
        //
        // Earlier versions also slept the full timeout AGAIN after a real
        // input arrived, which dragged every periodic block by its full
        // polling window after each reaction. We don't do that anymore.
        let millis = timeout.as_millis() as u64;
        let (result, _, _) = select_all([
            async {
                sleep_millis(millis).await;
                None
            }
            .boxed_local(),
            async {
                match self.read_inputs().await {
                    Some(idx) => Some(idx),
                    // No-input case: pend forever so the sleep branch wins.
                    None => std::future::pending::<Option<usize>>().await,
                }
            }
            .boxed_local(),
        ])
        .await;
        result
    }
}

/// Reads all inputs and awaits for any of them to have data
/// On the first input that has data, read the data and update
/// the input's value.
///
/// If the input kind does not match the received Value kind, this would put the block in fault.
///
/// # Returns
/// The index of the input that was read with a valid value.
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

    let value = val?;

    if let Some(input) = inputs.get_mut(idx) {
        let expected = *input.kind();
        let actual = HaystackKind::from(&value);

        if expected != HaystackKind::Null && expected != actual {
            match convert_value_kind(value, expected, actual) {
                Ok(value) => input.set_value(value),
                Err(err) => {
                    log::error!("Error converting value: {}", err);
                    block.set_state(BlockState::Fault);
                }
            }
        } else {
            input.set_value(value);
        }
    } else {
        block.set_state(BlockState::Fault);
    }
    Some(idx)
}
