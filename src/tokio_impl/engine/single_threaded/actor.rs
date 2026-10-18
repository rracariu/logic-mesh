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

        // Drive one step: execute completes or panics, a mailbox cmd
        // arrives (cancelling execute mid-await), or the gate flips. The
        // step also clears a fault optimistically before executing, and
        // puts it back unless the cycle completes (see `actor_step`), so
        // only a completed cycle ever flips a faulted block to Running.
        let step = run_one_step(&mut block, &mut mailbox, &mut gate, open, &mut hold).await;

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
        // Unbounded channel: `send` is synchronous and only fails once
        // the receiver is gone — the watcher went away without
        // unsubscribing (a closed UI tab). Drop it, or every actor would
        // keep computing changes for nobody.
        notification_channels
            .borrow_mut()
            .retain(|watcher, sender| {
                let sent = sender
                    .send(WatchMessage::new(
                        *block.id(),
                        changes.clone(),
                        state.clone(),
                    ))
                    .is_ok();
                if !sent {
                    log::debug!("Dropping watcher {watcher}: its receiver is gone");
                }
                sent
            });
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::time::Duration;

    use assert_matches::assert_matches;
    use libhaystack::val::kind::HaystackKind;
    use tokio::{
        sync::{mpsc, oneshot, watch},
        task::LocalSet,
        time::{sleep, timeout},
    };
    use uuid::Uuid;

    use super::block_actor_task;
    use crate::base::Status;
    use crate::base::block::connect::connect_output;
    use crate::base::engine::messages::BlockDefinition;
    use crate::base::input::Input;
    use crate::blocks::{InputImpl, math::Add};
    use crate::tokio_impl::engine::block_mailbox::{BLOCK_MAILBOX_CAP, BlockMailboxCmd};
    use crate::tokio_impl::engine::test_support::faults::{FaultOnce, REASON};

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

    /// Regression: a watcher whose receiver was dropped without
    /// unsubscribing stayed in the map, so every actor kept computing
    /// changes for it.
    #[tokio::test(flavor = "current_thread")]
    async fn a_watcher_whose_receiver_is_gone_is_dropped() {
        let local = LocalSet::new();
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (_gate, gate_rx) = watch::channel(true);
        let watchers = super::WatchersHandle::default();
        let (live, gone) = (Uuid::new_v4(), Uuid::new_v4());
        let (live_tx, mut live_rx) = mpsc::unbounded_channel();
        let (gone_tx, gone_rx) = mpsc::unbounded_channel();
        drop(gone_rx);
        watchers.borrow_mut().insert(live, live_tx);
        watchers.borrow_mut().insert(gone, gone_tx);
        let _actor = local.spawn_local(block_actor_task(
            Add::new(),
            mailbox_rx,
            watchers.clone(),
            gate_rx,
        ));

        // A pin write is a change every watcher is told about.
        let (reply, written) = oneshot::channel();
        mailbox_tx
            .try_send(BlockMailboxCmd::WriteInput {
                name: "in0".to_string(),
                value: 1.into(),
                reply,
            })
            .unwrap_or_else(|_| panic!("the mailbox has room"));
        let written = local.run_until(timeout(Duration::from_secs(2), written));
        written
            .await
            .expect("answered")
            .expect("replied")
            .expect("written");
        let notified = local.run_until(timeout(Duration::from_secs(2), live_rx.recv()));
        notified.await.expect("the live watcher is notified");

        assert_eq!(
            watchers.borrow().keys().copied().collect::<Vec<_>>(),
            vec![live],
            "only the live watcher is kept"
        );
    }

    /// Regression: the optimistic fault recovery was only undone when
    /// the gate flip itself ended the step. With a command already queued
    /// when the engine paused, the command won the biased select instead,
    /// and the block stayed Running — its outputs flipped to
    /// `Status::Ok` — for the whole pause.
    #[tokio::test(flavor = "current_thread")]
    async fn pausing_with_a_command_queued_keeps_the_fault() {
        a_queued_command_keeps_the_fault(true).await;
    }

    /// Regression: with the gate open, a command that cut a faulted
    /// block's cycle short left it Running, with `Status::Ok` outputs,
    /// until a later cycle faulted it again.
    #[tokio::test(flavor = "current_thread")]
    async fn a_command_keeps_the_fault() {
        a_queued_command_keeps_the_fault(false).await;
    }

    /// A block in an ordinary fault, recovering optimistically into a
    /// cycle that never completes, gets two inspections — the first one
    /// queued as the engine pauses, if `pause`. Both must read the fault,
    /// and downstream must never see `Status::Ok`.
    async fn a_queued_command_keeps_the_fault(pause: bool) {
        let local = LocalSet::new();
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (gate, gate_rx) = watch::channel(true);
        let mut block = FaultOnce::new();
        let mut downstream = InputImpl::new("in", HaystackKind::Number, Uuid::new_v4());
        connect_output(&mut block.out, &mut downstream).expect("linked");
        let _actor = local.spawn_local(block_actor_task(
            block,
            mailbox_rx,
            Default::default(),
            gate_rx,
        ));

        // The block faults, its output tells downstream, and the actor
        // recovers optimistically into a cycle that never completes.
        local.run_until(sleep(Duration::from_millis(50))).await;
        assert_matches!(downstream.try_take(), Some((_, Status::Fault)));

        // Queue an inspection — after pausing, if asked — before the
        // actor runs again, so it wakes to both at once.
        if pause {
            gate.send(false).expect("the actor holds the gate");
        }
        for _ in 0..2 {
            let inspected = queue_inspect(&mailbox_tx);
            let state = local.run_until(timeout(Duration::from_secs(2), inspected));
            let state = state.await.expect("answered").expect("replied");
            assert_eq!(state.state, "fault");
            assert_eq!(state.fault_reason.as_deref(), Some(REASON));
            local.run_until(sleep(Duration::from_millis(50))).await;
        }
        assert_matches!(downstream.try_take(), None, "downstream saw no Ok");
    }

    /// Queues an [`Inspect`](BlockMailboxCmd::Inspect) without driving the
    /// actor, returning where its answer arrives.
    fn queue_inspect(
        mailbox: &mpsc::Sender<BlockMailboxCmd>,
    ) -> oneshot::Receiver<BlockDefinition> {
        let (reply, inspected) = oneshot::channel();
        mailbox
            .try_send(BlockMailboxCmd::Inspect { reply })
            .unwrap_or_else(|_| panic!("the mailbox has room"));
        inspected
    }
}
