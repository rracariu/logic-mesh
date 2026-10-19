// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use futures::future::select_all;
use libhaystack::val::{Value, kind::HaystackKind};

use super::sleep::sleep_millis;
use crate::base::Status;
use crate::base::block::{Block, BlockState, convert_value_kind};
use crate::base::error::ErrorChain;
use crate::base::input::{Input, InputReader};
use crate::tokio_impl::scan::scan_interval_millis;
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Writes `value` to `input` the way an engine pin write does, with
/// [`Status::Ok`] — the value comes from the engine, not from a
/// producer block.
///
/// The value lands in the input's cache at once, so everything reading
/// the cache — `execute`, an inspection, the input's chained links —
/// sees it even while the block is held. It is also sent through the
/// input's own watch channel, which [`drain_ready_inputs`] reads whether
/// or not the input is linked, so the block's next input read returns
/// with it and the block runs one `execute` cycle with the new value: a
/// reactive block recomputes after a write to a constant input just as
/// it does after a linked one changes.
pub(crate) fn write_input_value<I>(input: &mut I, value: Value)
where
    I: Input<Reader = ReaderImpl, Writer = WriterImpl> + ?Sized,
{
    input.set_value(value.clone(), Status::Ok);
    input.writer().send_replace((value, Status::Ok));
}

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
        let mut backoff = scan_interval_millis();
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
/// - If any input already has a fresh value — a linked one, or any input
///   an engine pin write reached (see [`write_input_value`]) — drain them
///   all synchronously and return the index of the last one drained.
/// - Otherwise, await `receiver()` on every connected input and, when one
///   wakes up, drain again (the wait might have woken several at once).
///   Unlinked inputs need no waiting: only an engine pin write reaches
///   them, and that write arrives as a mailbox command, which ends the
///   block's in-flight `execute` — the next one drains it in the first
///   step.
/// - Returns [`None`] only when nothing was fresh and the block has no
///   connected inputs at all.
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
/// index of the last drained input (or [`None`] if nothing was ready).
///
/// Every input is read, linked or not: an unlinked input's channel only
/// changes on an engine pin write, which must run the block like any
/// other fresh value.
///
/// Faults the block if:
/// - any value fails type conversion (Phase 1: block-level fault), or
/// - any drained payload carries [`Status::Fault`] — upstream fault
///   propagation, the consumer enters Fault until the upstream recovers
///   and pushes [`Status::Ok`] on a later cycle.
pub(crate) fn drain_ready_inputs<B: Block>(block: &mut B) -> Option<usize> {
    let mut last_idx = None;
    let mut conversion_fault: Option<String> = None;
    let mut upstream_fault: Option<String> = None;

    {
        let mut inputs = block.inputs_mut();
        for (idx, input) in inputs.iter_mut().enumerate() {
            let Some((value, status)) = input.try_take() else {
                continue;
            };

            let expected = *input.kind();
            let actual = HaystackKind::from(&value);
            let converted = if expected != HaystackKind::Null && expected != actual {
                match convert_value_kind(value, expected, actual) {
                    Ok(v) => v,
                    Err(err) => {
                        // The decoder's own error is the source, so
                        // render the chain to keep its detail.
                        let err = ErrorChain(&err);
                        log::error!("Error converting value: {err}");
                        if conversion_fault.is_none() {
                            conversion_fault = Some(format!(
                                "type conversion failed on input {}: {err}",
                                input.name(),
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

#[cfg(test)]
mod test {
    use libhaystack::val::Value;

    use super::drain_ready_inputs;
    use crate::base::block::{BlockProps, test_utils::write_block_inputs};
    use crate::blocks::bitwise::BitwiseNot;

    /// Regression: a Zinc decoding failure keeps the decoder's error
    /// only as its source, and the fault reason printed the error alone,
    /// so it lost the part saying what was wrong with the string.
    #[tokio::test]
    async fn conversion_fault_reason_keeps_the_decoder_detail() {
        let mut block = BitwiseNot::new();
        write_block_inputs([(&mut block.input, Value::make_str("not zinc ("))]).await;

        drain_ready_inputs(&mut block);

        let detail = libhaystack::encoding::zinc::decode::from_str("not zinc (")
            .expect_err("not zinc")
            .to_string();
        assert_eq!(
            block.state().fault_reason(),
            Some(
                format!("type conversion failed on input in: Zinc decoding failed: {detail}")
                    .as_str()
            )
        );
    }
}
