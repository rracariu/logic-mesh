// Copyright (c) 2022-2026, Radu Racariu.

//! One step of a block actor, shared by both engines' actor loops.
//!
//! A step waits for whichever comes first: a mailbox command, the end
//! of one `execute()` cycle, or a flip of the engine's execution gate.
//! The gate is open while the engine runs and is not paused; while it is
//! closed the block never executes, but its mailbox is still served, so
//! every engine request keeps getting answered.
//!
//! ## Fault recovery
//!
//! A block in Fault is cleared back to Running just before it executes,
//! optimistically, and the cycle sets Fault again if the cause persists
//! (see [`recover_optimistically`]). Only a cycle that completes can
//! clear a fault: one cut short by a mailbox command or by the gate
//! closing puts the fault back, so a request, a pause or a shutdown
//! never flips a faulted block, or its outputs, to Ok.
//!
//! ## Panicking blocks
//!
//! On native targets a panic inside `execute()` is caught and isolated
//! to its block: the actor stays alive, so the block can still be
//! inspected, saved, rewired, removed and terminated. The block enters
//! [`BlockState::Fault`] with the panic message as its reason, and the
//! actor loop's status propagation marks every output
//! [`Status::Fault`](crate::base::Status::Fault), so downstream blocks
//! fault in turn.
//!
//! The block is then held (see [`PanicHold`]): it is not executed again
//! until a retry delay has passed, and the delay doubles with every
//! consecutive panic, from [`PANIC_RETRY_INITIAL`] up to
//! [`PANIC_RETRY_MAX`] — so a block that panics on every cycle logs one
//! error per delay rather than flooding the logs. A held block skips
//! the optimistic fault recovery: it stays in Fault until an `execute()`
//! cycle runs to completion without panicking — typically once an input
//! changes, since most blocks wait on their inputs first — and then
//! returns to Running. A reset or removal ends the hold with the block.
//!
//! The block is reused after a panic — the unwind is asserted safe — so
//! a panic that leaves it half-updated is not undone; the Fault state
//! is what tells the rest of the program not to trust its values.
//!
//! On `wasm32` a panic aborts the whole instance, so there is nothing
//! to catch and the isolation is native-only.

use std::pin::Pin;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::Sleep;

use super::block_mailbox::{BlockMailboxCmd, handle_cmd};
use crate::base::block::BlockState;
use crate::tokio_impl::EngineBlock;

/// An actor's view of its engine's execution gate: `true` while the
/// engine lets its blocks execute.
pub(super) type ExecutionGate = watch::Receiver<bool>;

/// How long a block that panicked is held before it executes again.
pub(super) const PANIC_RETRY_INITIAL: Duration = Duration::from_secs(1);

/// The longest a block that keeps panicking is held between retries.
pub(super) const PANIC_RETRY_MAX: Duration = Duration::from_secs(60);

/// What a call to [`run_one_step`] did.
pub(super) enum Step {
    /// A mailbox command was handled — or the mailbox closed. `true`
    /// if the actor must exit.
    Command(bool),
    /// One `execute()` cycle ran to completion.
    Executed,
    /// One `execute()` cycle panicked; the block is now held.
    Panicked,
    /// The execution gate flipped, or a panic retry delay ran out,
    /// before anything else happened. Any in-flight `execute()` was
    /// dropped (see the cancellation-safety notes in the single-threaded
    /// engine docs).
    Woken,
}

/// Holds a block after a panic: in Fault, and not executed until a
/// retry delay has passed. See the [module docs](self).
pub(super) struct PanicHold {
    /// The fault the last panic put the block in, while it is held.
    fault: Option<BlockState>,
    /// The retry delay currently running, if any.
    retry: Option<Pin<Box<Sleep>>>,
    /// The delay to apply on the next panic.
    next_delay: Duration,
}

impl Default for PanicHold {
    fn default() -> Self {
        Self {
            fault: None,
            retry: None,
            next_delay: PANIC_RETRY_INITIAL,
        }
    }
}

impl PanicHold {
    /// `true` from a panic until an `execute()` cycle completes cleanly.
    fn is_holding(&self) -> bool {
        self.fault.is_some()
    }

    /// `true` while the retry delay is still running.
    fn is_waiting(&self) -> bool {
        self.retry.as_ref().is_some_and(|retry| !retry.is_elapsed())
    }

    /// Resolves once the retry delay has run out.
    async fn retry_elapsed(&mut self) {
        if let Some(retry) = self.retry.as_mut() {
            retry.as_mut().await;
        }
        self.retry = None;
    }

    /// Faults `block` with the panic `message` and starts the retry delay.
    fn hold<B: EngineBlock>(&mut self, block: &mut B, message: &str) {
        let delay = self.next_delay;
        self.next_delay = (delay * 2).min(PANIC_RETRY_MAX);
        log::error!(
            "Block {} ({}) panicked: {message}; holding it in Fault, next retry in {delay:?}",
            block.id(),
            block.name(),
        );

        let fault = BlockState::fault(format!("block panicked: {message}"));
        block.set_state(fault.clone());
        self.fault = Some(fault);
        self.retry = Some(Box::pin(tokio::time::sleep(delay)));
    }

    /// Ends the hold after a clean cycle. The block returns to Running
    /// unless the cycle left it in a fault of its own.
    fn release<B: EngineBlock>(&mut self, block: &mut B) {
        let Some(fault) = self.fault.take() else {
            return;
        };
        self.next_delay = PANIC_RETRY_INITIAL;
        if block.state() == fault {
            block.set_state(BlockState::Running);
        }
        log::info!(
            "Block {} ({}) recovered from a panic",
            block.id(),
            block.name()
        );
    }
}

/// What ended a step that started an `execute()` cycle.
enum Race {
    /// The cycle finished; `Err` holds its panic message.
    Finished(Result<(), String>),
    /// A mailbox command arrived first — or the mailbox closed.
    Command(Option<BlockMailboxCmd>),
    /// The gate closed first.
    GateClosed,
}

/// Drives `block` one step. `open` is the gate state the caller read
/// for this step: the block only executes if it is `true` and `hold` is
/// not waiting out a retry delay, and a gate that has closed since then
/// ends the step as [`Step::Woken`].
pub(super) async fn run_one_step<B>(
    block: &mut B,
    mailbox: &mut mpsc::Receiver<BlockMailboxCmd>,
    gate: &mut ExecutionGate,
    open: bool,
    hold: &mut PanicHold,
) -> Step
where
    B: EngineBlock + 'static,
{
    let cmd = if open && !hold.is_waiting() {
        let recovering_from = recover_optimistically(block, hold);
        let race = {
            let execute = execute_guarded(block);
            tokio::pin!(execute);
            tokio::select! {
                biased;
                // Prefer mailbox so external commands aren't delayed by a
                // block that's perpetually ready to execute, and the gate
                // over execute so a paused block never completes a cycle.
                cmd = mailbox.recv() => Race::Command(cmd),
                () = wait_for_gate(gate, false) => Race::GateClosed,
                finished = &mut execute => Race::Finished(finished),
            }
            // `execute` goes out of scope here — its borrow on block ends.
        };
        match race {
            Race::Finished(Ok(())) => {
                hold.release(block);
                return Step::Executed;
            }
            Race::Finished(Err(message)) => {
                hold.hold(block, &message);
                return Step::Panicked;
            }
            // The cycle was dropped before it completed, so it proved
            // nothing: put the fault back before anything — the command
            // included — reads the state.
            Race::Command(cmd) => {
                undo_recovery(block, recovering_from);
                cmd
            }
            Race::GateClosed => {
                undo_recovery(block, recovering_from);
                return Step::Woken;
            }
        }
    } else {
        tokio::select! {
            biased;
            cmd = mailbox.recv() => cmd,
            () = wait_for_gate(gate, true), if !open => return Step::Woken,
            () = hold.retry_elapsed(), if hold.is_waiting() => return Step::Woken,
        }
    };

    match cmd {
        Some(cmd) => Step::Command(handle_cmd(cmd, block).await),
        // Every sender is gone — the engine dropped this block's handle
        // (or the engine itself was dropped), so nothing can reach the
        // block again. A closed mailbox is always ready, so staying in
        // the loop would let it win every biased select and spin
        // forever without ever executing the block.
        None => {
            block.set_state(BlockState::Terminated);
            Step::Command(true)
        }
    }
}

/// Optimistic recovery, for a block about to execute: if a previous
/// cycle left the block in Fault, clear it back to Running and return
/// that fault. If the upstream fault persists or the block re-faults,
/// drain or execute set Fault again during the cycle; otherwise the
/// cycle completes with the block Running, and the actor loop's
/// end-of-step `emit_status(Ok)` broadcasts the recovery. A cycle that
/// does not complete hands the fault to [`undo_recovery`].
///
/// We deliberately do NOT reset each output's `effective_status` here.
/// Doing so would mean `output.set(value)` calls inside `execute()` emit
/// `(value, Ok)` even when the block is about to re-enter Fault this
/// same cycle — a non-conservative status flicker that can mask a fault.
/// Keeping `effective_status` sticky between cycles means: while in
/// Fault, value updates ride out as `(value, Fault)`; on recovery,
/// `emit_status(Ok)` at the end of the recovery cycle is the single
/// authoritative flip.
///
/// A block held after a panic does not recover here: it stays in Fault
/// until a cycle completes cleanly (see the [module docs](self)).
fn recover_optimistically<B: EngineBlock>(block: &mut B, hold: &PanicHold) -> Option<BlockState> {
    let state = block.state();
    if hold.is_holding() || !state.is_fault() {
        return None;
    }
    block.set_state(BlockState::Running);
    Some(state)
}

/// Puts back the fault [`recover_optimistically`] cleared, for a cycle
/// that was dropped before it completed: only a completed cycle can
/// clear a fault, so a mailbox command, a pause or a shutdown never
/// flips a faulted block — and its outputs — to Ok. A state the
/// dropped cycle set itself, other than Running, is kept.
fn undo_recovery<B: EngineBlock>(block: &mut B, recovering_from: Option<BlockState>) {
    if let Some(fault) = recovering_from
        && block.state() == BlockState::Running
    {
        block.set_state(fault);
    }
}

/// Runs one `execute()` cycle, returning the panic message if it
/// panicked.
async fn execute_guarded<B>(block: &mut B) -> Result<(), String>
where
    B: EngineBlock + 'static,
{
    catch_panic(block.execute()).await
}

/// Awaits `future`, returning the message of a panic raised while it
/// is polled instead of unwinding into the caller.
///
/// The future is asserted unwind safe: the caller must not trust what
/// a panic may have left half-updated.
#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn catch_panic<F: Future>(future: F) -> Result<F::Output, String> {
    use futures::FutureExt;

    std::panic::AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(|payload| panic_message(payload.as_ref()))
}

/// Awaits `future`. A panic aborts a `wasm32` instance, so there is
/// none to catch.
#[cfg(target_arch = "wasm32")]
pub(super) async fn catch_panic<F: Future>(future: F) -> Result<F::Output, String> {
    Ok(future.await)
}

/// The message a panic was raised with: `panic!` payloads are a `&str`
/// or, when formatted, a `String`.
#[cfg(not(target_arch = "wasm32"))]
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

/// Resolves once the gate reads `open`.
async fn wait_for_gate(gate: &mut ExecutionGate, open: bool) {
    let engine_gone = gate.wait_for(|is_open| *is_open == open).await.is_err();
    if engine_gone {
        // The gate's sender lives in the engine, next to this block's
        // mailbox sender: with the engine gone the mailbox closes too,
        // and the mailbox arm ends the actor. Never resolve here, or
        // the closed gate would win the select on every step.
        std::future::pending::<()>().await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use super::panic_message;

    #[test]
    fn panic_message_reads_str_and_string_payloads() {
        let literal = std::panic::catch_unwind(|| panic!("literal")).expect_err("panics");
        let formatted =
            std::panic::catch_unwind(|| panic!("formatted {}", 42)).expect_err("panics");
        let other = std::panic::catch_unwind(|| std::panic::panic_any(7_u8)).expect_err("panics");

        assert_eq!(panic_message(literal.as_ref()), "literal");
        assert_eq!(panic_message(formatted.as_ref()), "formatted 42");
        assert_eq!(panic_message(other.as_ref()), "non-string panic payload");
    }
}
