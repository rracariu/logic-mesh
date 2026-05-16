// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use futures::future::select_all;
use libhaystack::val::kind::HaystackKind;

use super::sleep::sleep_millis;
use crate::base::Status;
use crate::base::block::{Block, BlockState, convert_value_kind};
use crate::base::input::{InputProps, input_reader::InputReader};
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
        //
        // Implementation note: we use `tokio::select!` rather than
        // `select_all([fut.boxed_local(), ...])` so the resulting future
        // inherits `Send` from the bodies. With `boxed_local()` the whole
        // thing was `!Send`, which forced the MT engine onto per-worker
        // `LocalSet`s. Now it can ride directly on `tokio::spawn`.
        let millis = timeout.as_millis() as u64;
        tokio::select! {
            _ = sleep_millis(millis) => None,
            result = async {
                loop {
                    match self.read_inputs().await {
                        Some(idx) => return Some(idx),
                        // No connected inputs — pend forever so the sleep
                        // arm wins. A bare `pending::<()>()` here would
                        // make the arm `!Send` on some toolchains; the
                        // explicit type annotation keeps inference happy.
                        None => std::future::pending::<()>().await,
                    }
                }
            } => result,
        }
    }
}

/// Reads all of the block's inputs, returning when at least one has a fresh
/// value to expose. Drains *every* input that already has a pending update,
/// not just one — so a multi-input block (PID, Reset, Enthalpy, …) sees a
/// temporally-coherent snapshot per cycle rather than being walked one input
/// at a time.
///
/// Behavior:
/// - If any connected input already has a fresh value, drain them all
///   synchronously and return the index of the last one drained.
/// - Otherwise, await `receiver()` on every connected input and, when one
///   wakes up, drain again (the wait might have woken several at once).
/// - Returns `None` only when the block has no connected inputs at all.
///
/// Type-mismatched inputs are converted via [`convert_value_kind`]; if the
/// conversion fails the block transitions to [`BlockState::Fault`].
pub(crate) async fn read_block_inputs<B: Block>(block: &mut B) -> Option<usize> {
    // Phase 1: drain anything already ready, no awaiting.
    if let Some(idx) = drain_ready_inputs(block) {
        return Some(idx);
    }

    // Phase 2: nothing was ready — await for at least one connected input
    // to change. We collect a fresh `inputs_mut()` borrow inside a block so
    // the futures (and their `&mut` borrows) are released before phase 3
    // re-borrows the block.
    {
        let mut connected: Vec<_> = block
            .inputs_mut()
            .into_iter()
            .filter(|i| i.is_connected())
            .collect();
        if connected.is_empty() {
            return None;
        }
        let futures: Vec<_> = connected.iter_mut().map(|i| i.receiver()).collect();
        let _ = select_all(futures).await;
    }

    // Phase 3: drain everything that changed during the await — multiple
    // inputs may have woken concurrently.
    drain_ready_inputs(block)
}

/// Synchronously drain every input that has a fresh payload. Returns the
/// index of the last drained input (or `None` if nothing was ready).
///
/// Faults the block if:
/// - any value fails type conversion (Phase 1: block-level fault), or
/// - any drained payload carries [`Status::Fault`] — upstream fault
///   propagation, the consumer enters Fault until the upstream recovers
///   and pushes `Status::Ok` on a later cycle.
fn drain_ready_inputs<B: Block>(block: &mut B) -> Option<usize> {
    let mut last_idx = None;
    let mut conversion_fault: Option<String> = None;
    let mut upstream_fault: Option<String> = None;

    {
        let mut inputs = block.inputs_mut();
        for (idx, input) in inputs.iter_mut().enumerate() {
            if !input.is_connected() {
                continue;
            }
            let Some((value, status)) = input.try_take() else {
                continue;
            };

            let expected = *input.kind();
            let actual = HaystackKind::from(&value);
            let converted = if expected != HaystackKind::Null && expected != actual {
                match convert_value_kind(value, expected, actual) {
                    Ok(v) => v,
                    Err(err) => {
                        log::error!("Error converting value: {}", err);
                        if conversion_fault.is_none() {
                            conversion_fault = Some(format!(
                                "type conversion failed on input {}: {}",
                                input.name(),
                                err
                            ));
                        }
                        continue;
                    }
                }
            } else {
                value
            };

            // Carry upstream status with the value into the input's cache
            // and onward to any chained input links.
            input.set_value(converted, status);

            if status == Status::Fault && upstream_fault.is_none() {
                upstream_fault = Some(format!("upstream fault on input {}", input.name()));
            }

            last_idx = Some(idx);
        }
    }

    // Conversion failures take precedence — they're our own problem and
    // more actionable for the user than "the thing feeding me is broken."
    if let Some(reason) = conversion_fault {
        block.set_state(BlockState::fault(reason));
    } else if let Some(reason) = upstream_fault {
        block.set_state(BlockState::fault(reason));
    }
    last_idx
}
