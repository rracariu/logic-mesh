// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block actor task.
//!
//! Each scheduled block lives in a task spawned via [`block_actor_task`].
//! The task **owns the block by value** — there is no `Rc`, no `UnsafeCell`,
//! and no aliasing. The task loop interleaves `block.execute()` with mailbox
//! handling via `tokio::select!`; when a mailbox command arrives, the
//! in-flight execute future is dropped (cancellation-safe — see the module
//! docstring) and the command is handled before a fresh `execute()` is
//! started.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use libhaystack::val::Value;
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;

use super::super::block_mailbox::{BlockMailboxCmd, handle_cmd};
use crate::base::Status;
use crate::base::block::{Block, BlockState};
use crate::base::engine::messages::{ChangeSource, WatchMessage};
use crate::tokio_impl::EngineBlock;

/// ST-side watchers handle: single-threaded, no thread-safety needed.
pub(super) type WatchersHandle = Rc<RefCell<BTreeMap<Uuid, UnboundedSender<WatchMessage>>>>;

/// Per-block actor task. Owns the block by value; processes mailbox commands
/// interleaved with `block.execute()` cycles.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
) where
    B: EngineBlock + 'static,
{
    let mut last_pin_values = BTreeMap::<String, Value>::new();
    let mut last_state = BlockState::Running;
    let mut terminated = false;

    while !terminated {
        // Optimistic recovery: if the previous cycle left the block in
        // Fault, clear back to Running before drain/execute. If the
        // upstream fault persists or the block re-faults during execute,
        // drain or execute will re-set Fault on this cycle; otherwise the
        // block recovers naturally and the end-of-cycle `emit_status(Ok)`
        // broadcasts the recovery.
        //
        // We deliberately do NOT reset each output's `effective_status`
        // here. Doing so would mean `output.set(value)` calls inside
        // `execute()` emit `(value, Ok)` even when the block is about to
        // re-enter Fault this same cycle — a non-conservative status
        // flicker that can mask a fault. Keeping `effective_status`
        // sticky between cycles means: while in Fault, value updates ride
        // out as `(value, Fault)`; on recovery, `emit_status(Ok)` at the
        // end of the recovery cycle is the single authoritative flip.
        if block.state().is_fault() {
            block.set_state(BlockState::Running);
        }

        // Drive one step: either execute completes, or a mailbox cmd arrives
        // (cancelling execute mid-await).
        terminated = run_one_step(&mut block, &mut mailbox).await;

        // Propagate state changes to output statuses. emit_status is a no-op
        // on the wire when nothing changed (send_if_modified comparison).
        let current_state = block.state();
        propagate_status(&current_state, &last_state, &mut block);

        change_of_value_check(
            &watchers,
            &block,
            &mut last_pin_values,
            &current_state,
            &last_state,
        );

        last_state = current_state;

        if matches!(block.state(), BlockState::Terminated) {
            break;
        }

        // Co-operative yield: prevents a tight-cycling block (e.g. a
        // misconfigured periodic block with no awaits) from starving siblings
        // on the same LocalSet.
        tokio::task::yield_now().await;
    }
}

/// Push status updates to every output when the block's state changes
/// between Fault and not-Fault. Recovery (Fault → Running) also re-emits
/// `Ok` so consumers see the recovery even if the value didn't change.
fn propagate_status<B>(current: &BlockState, previous: &BlockState, block: &mut B)
where
    B: EngineBlock + 'static,
{
    let now_fault = current.is_fault();
    let was_fault = previous.is_fault();

    if now_fault {
        for output in block.outputs_mut() {
            output.emit_status(Status::Fault);
        }
    } else if was_fault {
        for output in block.outputs_mut() {
            output.emit_status(Status::Ok);
        }
    }
}

/// Drive the block one step. Returns `true` if the actor task should exit.
async fn run_one_step<B>(block: &mut B, mailbox: &mut mpsc::Receiver<BlockMailboxCmd>) -> bool
where
    B: EngineBlock + 'static,
{
    let mut cmd_to_handle: Option<BlockMailboxCmd> = None;
    {
        let execute_fut = block.execute();
        tokio::pin!(execute_fut);
        tokio::select! {
            biased;
            // Prefer mailbox so external commands aren't delayed by
            // a block that's perpetually ready to execute.
            cmd = mailbox.recv() => {
                cmd_to_handle = cmd;
            }
            () = &mut execute_fut => {}
        }
        // execute_fut goes out of scope here — its borrow on block ends.
    }

    let Some(cmd) = cmd_to_handle else {
        return false;
    };

    handle_cmd(cmd, block).await
}

/// Detect changes on the block's pins and/or state relative to the
/// previously-emitted snapshot and dispatch a `WatchMessage` to every
/// subscribed watcher. The current `BlockState` rides on every emitted
/// message so the UI can render Fault state alongside the pin updates.
fn change_of_value_check<B: Block + 'static>(
    notification_channels: &WatchersHandle,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
    state: &BlockState,
    prev_state: &BlockState,
) {
    if notification_channels.borrow().is_empty() {
        if !last_pin_values.is_empty() {
            last_pin_values.clear();
        }
        return;
    }

    let mut changes = HashMap::<String, ChangeSource>::new();

    block.outputs().iter().for_each(|output| {
        let pin = output.desc().name.to_string();
        let val = output.value();
        if last_pin_values.get(&pin) != Some(val) {
            changes.insert(pin.clone(), ChangeSource::Output(pin.clone(), val.clone()));
            last_pin_values.insert(pin, val.clone());
        }
    });

    block.inputs().iter().for_each(|input| {
        let val = input.get_value();
        if let Some(val) = val {
            let pin = input.name().to_string();
            if last_pin_values.get(&pin) != Some(val) {
                changes.insert(pin.clone(), ChangeSource::Input(pin.clone(), val.clone()));
                last_pin_values.insert(pin, val.clone());
            }
        }
    });

    // Emit when either a pin value changed OR the block's state did.
    // Without the state-change check, a block entering Fault with no
    // value change (e.g., upstream stopped emitting) would be invisible.
    if !changes.is_empty() || state != prev_state {
        // Unbounded channel: `send` is synchronous and only fails on
        // receiver-dropped (channel closed). We ignore the error path
        // because a dropped watcher means the UI tab closed.
        for sender in notification_channels.borrow().values() {
            let _ = sender.send(WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
                state: state.clone(),
            });
        }
    }
}
