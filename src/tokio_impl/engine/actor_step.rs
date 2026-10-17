// Copyright (c) 2022-2026, Radu Racariu.

//! One step of a block actor, shared by both engines' actor loops.
//!
//! A step waits for whichever comes first: a mailbox command, the end
//! of one `execute()` cycle, or a flip of the engine's execution gate.
//! The gate is open while the engine runs and is not paused; while it is
//! closed the block never executes, but its mailbox is still served, so
//! every engine request keeps getting answered.

use tokio::sync::{mpsc, watch};

use super::block_mailbox::{BlockMailboxCmd, handle_cmd};
use crate::base::block::BlockState;
use crate::tokio_impl::EngineBlock;

/// An actor's view of its engine's execution gate: `true` while the
/// engine lets its blocks execute.
pub(super) type ExecutionGate = watch::Receiver<bool>;

/// What a call to [`run_one_step`] did.
pub(super) enum Step {
    /// A mailbox command was handled — or the mailbox closed. `true`
    /// if the actor must exit.
    Command(bool),
    /// One `execute()` cycle ran to completion.
    Executed,
    /// The execution gate flipped before anything else happened. Any
    /// in-flight `execute()` was dropped (see the cancellation-safety
    /// notes in the single-threaded engine docs).
    Gated,
}

/// Drives `block` one step. `open` is the gate state the caller read
/// for this step: the block only executes if it is `true`, and a gate
/// that has closed since then ends the step as [`Step::Gated`].
pub(super) async fn run_one_step<B>(
    block: &mut B,
    mailbox: &mut mpsc::Receiver<BlockMailboxCmd>,
    gate: &mut ExecutionGate,
    open: bool,
) -> Step
where
    B: EngineBlock + 'static,
{
    let cmd = if open {
        let execute = block.execute();
        tokio::pin!(execute);
        tokio::select! {
            biased;
            // Prefer mailbox so external commands aren't delayed by a
            // block that's perpetually ready to execute, and the gate
            // over execute so a paused block never completes a cycle.
            cmd = mailbox.recv() => cmd,
            () = wait_for_gate(gate, false) => return Step::Gated,
            () = &mut execute => return Step::Executed,
        }
        // `execute` goes out of scope here — its borrow on block ends.
    } else {
        tokio::select! {
            biased;
            cmd = mailbox.recv() => cmd,
            () = wait_for_gate(gate, true) => return Step::Gated,
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
