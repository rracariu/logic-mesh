// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block actor task.
//!
//! Each scheduled block lives in a task spawned via [`block_actor_task`].
//! The task **owns the block by value** — there is no
//! [`Rc`](std::rc::Rc), no [`UnsafeCell`](std::cell::UnsafeCell), and no
//! aliasing. The task loop interleaves `block.execute()` with mailbox
//! handling via [`tokio::select!`] (one step at a time, see
//! `actor_step`); when a mailbox command arrives, the in-flight execute
//! future is dropped (cancellation-safe — see the module docstring) and
//! the command is handled before a fresh `execute()` is started.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use libhaystack::val::Value;
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;

use super::super::actor_step::{ExecutionGate, PanicHold, Step, run_one_step};
use super::super::block_mailbox::BlockMailboxCmd;
use crate::base::Status;
use crate::base::block::{Block, BlockState};
use crate::base::engine::messages::{ChangeSource, WatchMessage};
use crate::tokio_impl::EngineBlock;

/// ST-side watchers handle: single-threaded, no thread-safety needed.
pub(super) type WatchersHandle = Rc<RefCell<BTreeMap<Uuid, UnboundedSender<WatchMessage>>>>;

/// Per-block actor task. Owns the block by value; processes mailbox commands
/// interleaved with `block.execute()` cycles, which run only while the
/// engine's execution `gate` is open.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
    mut gate: ExecutionGate,
) where
    B: EngineBlock + 'static,
{
    let mut last_pin_values = BTreeMap::<String, Value>::new();
    let mut last_state = BlockState::Running;
    let mut hold = PanicHold::default();

    loop {
        let open = *gate.borrow_and_update();

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
        //
        // Only a block about to execute recovers: while the gate is
        // closed a fault stays put, so pausing never flips a status. A
        // block held after a panic stays in Fault until a cycle
        // completes cleanly (see `actor_step`).
        let recovering_from = if open && !hold.is_holding() && block.state().is_fault() {
            let fault = block.state();
            block.set_state(BlockState::Running);
            Some(fault)
        } else {
            None
        };

        // Drive one step: execute completes or panics, a mailbox cmd
        // arrives (cancelling execute mid-await), or the gate flips.
        let step = run_one_step(&mut block, &mut mailbox, &mut gate, open, &mut hold).await;
        if let (Step::Woken, Some(fault)) = (&step, recovering_from) {
            // The gate closed before the block got to run, so nothing
            // could have cleared the fault.
            block.set_state(fault);
        }

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

        if matches!(step, Step::Command(true)) || matches!(block.state(), BlockState::Terminated) {
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
/// [`Status::Ok`](crate::base::Status::Ok) so consumers see the recovery
/// even if the value didn't change.
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

/// Detect changes on the block's pins and/or state relative to the
/// previously-emitted snapshot and dispatch a [`WatchMessage`] to every
/// subscribed watcher. The current [`BlockState`] rides on every emitted
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

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::{
        sync::{mpsc, watch},
        task::LocalSet,
        time::timeout,
    };

    use super::block_actor_task;
    use crate::blocks::math::Add;
    use crate::tokio_impl::engine::block_mailbox::BLOCK_MAILBOX_CAP;

    /// Regression: a closed mailbox used to read as "keep running", and
    /// since it is always ready it won every biased select — the actor
    /// spun forever without executing the block.
    #[tokio::test(flavor = "current_thread")]
    async fn actor_exits_when_its_mailbox_closes() {
        let local = LocalSet::new();
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (_gate, gate_rx) = watch::channel(true);
        let task = local.spawn_local(block_actor_task(
            Add::new(),
            mailbox_rx,
            Default::default(),
            gate_rx,
        ));

        drop(mailbox_tx);

        local
            .run_until(timeout(Duration::from_secs(2), task))
            .await
            .expect("actor task exits once its mailbox closes")
            .expect("actor task completes without panicking");
    }
}
