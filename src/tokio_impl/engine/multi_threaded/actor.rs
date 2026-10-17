// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block actor task for the multi-threaded engine.
//!
//! Same shape as the single-threaded variant: the actor task **owns the
//! block by value** and [`tokio::select!`]s between `block.execute()` and a
//! per-block mailbox, one shared `actor_step` at a time. The MT-specific
//! differences are:
//!
//! - The block is sent across thread boundaries (worker thread) once, then
//!   stays put. `B: `[`Send`] is required at scheduling time, but the actor
//!   future itself is `!Send` (runs on the worker's [`LocalSet`](tokio::task::LocalSet)).
//! - Watchers are stored as `Arc<RwLock<...>>` (cross-worker visibility),
//!   so the change-of-value check is async.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, UnboundedSender},
};
use uuid::Uuid;

use super::super::actor_step::{ExecutionGate, PanicHold, Step, run_one_step};
use super::super::block_mailbox::BlockMailboxCmd;
use crate::base::Status;
use crate::base::block::{Block, BlockState};
use crate::base::engine::messages::{ChangeSource, WatchMessage};
use crate::tokio_impl::MtBlock;

/// MT-side watchers handle: cross-thread, async-locked.
pub(super) type WatchersHandle = Arc<RwLock<BTreeMap<Uuid, UnboundedSender<WatchMessage>>>>;

/// Per-block actor task. Owns the block by value; processes mailbox
/// commands interleaved with `block.execute()` cycles, which run only
/// while the engine's execution `gate` is open.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
    mut gate: ExecutionGate,
) where
    B: MtBlock + 'static,
{
    let mut last_pin_values = BTreeMap::<String, Value>::new();
    let mut last_state = BlockState::Running;
    let mut hold = PanicHold::default();

    loop {
        let open = *gate.borrow_and_update();

        // One step, optimistic fault recovery included — see
        // single_threaded::actor and `actor_step`.
        let step = run_one_step(&mut block, &mut mailbox, &mut gate, open, &mut hold).await;

        let current_state = block.state();
        propagate_status(&current_state, &last_state, &mut block);

        change_of_value_check(
            &watchers,
            &block,
            &mut last_pin_values,
            &current_state,
            &last_state,
        )
        .await;

        last_state = current_state;

        if matches!(step, Step::Command(true)) || matches!(block.state(), BlockState::Terminated) {
            break;
        }

        tokio::task::yield_now().await;
    }
}

fn propagate_status<B>(current: &BlockState, previous: &BlockState, block: &mut B)
where
    B: MtBlock + 'static,
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

async fn change_of_value_check<B: Block + 'static>(
    notification_channels: &WatchersHandle,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
    state: &BlockState,
    prev_state: &BlockState,
) {
    if notification_channels.read().await.is_empty() {
        if !last_pin_values.is_empty() {
            last_pin_values.clear();
        }
        return;
    }

    let mut changes = HashMap::<String, ChangeSource>::new();

    for output in block.outputs() {
        let pin = output.desc().name.to_string();
        let val = output.value();
        if last_pin_values.get(&pin) != Some(val) {
            changes.insert(pin.clone(), ChangeSource::Output(pin.clone(), val.clone()));
            last_pin_values.insert(pin, val.clone());
        }
    }

    for input in block.inputs() {
        if let Some(val) = input.get_value() {
            let pin = input.name().to_string();
            if last_pin_values.get(&pin) != Some(val) {
                changes.insert(pin.clone(), ChangeSource::Input(pin.clone(), val.clone()));
                last_pin_values.insert(pin, val.clone());
            }
        }
    }

    if !changes.is_empty() || state != prev_state {
        // Unbounded channel — see single_threaded/actor.rs rationale. A
        // failed send means the watcher went away without
        // unsubscribing; drop it under the write lock, sparing the
        // common path the exclusive lock.
        let mut gone = Vec::new();
        for (watcher, sender) in notification_channels.read().await.iter() {
            let message = WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
                state: state.clone(),
            };
            if sender.send(message).is_err() {
                gone.push(*watcher);
            }
        }
        if !gone.is_empty() {
            // The id may have subscribed again in between, with a live
            // channel: only drop a channel that is still closed.
            notification_channels
                .write()
                .await
                .retain(|watcher, sender| !(gone.contains(watcher) && sender.is_closed()));
            log::debug!("Dropped watchers {gone:?}: their receivers are gone");
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use assert_matches::assert_matches;
    use libhaystack::val::kind::HaystackKind;
    use tokio::{
        sync::{mpsc, oneshot, watch},
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

    /// Regression: see the single-threaded
    /// `actor_exits_when_its_mailbox_closes`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actor_exits_when_its_mailbox_closes() {
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (_gate, gate_rx) = watch::channel(true);
        let task = tokio::spawn(block_actor_task(
            Add::new(),
            mailbox_rx,
            Default::default(),
            gate_rx,
        ));

        drop(mailbox_tx);

        timeout(Duration::from_secs(2), task)
            .await
            .expect("actor task exits once its mailbox closes")
            .expect("actor task completes without panicking");
    }

    /// Regression: see the single-threaded
    /// `a_watcher_whose_receiver_is_gone_is_dropped`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_watcher_whose_receiver_is_gone_is_dropped() {
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (_gate, gate_rx) = watch::channel(true);
        let watchers = super::WatchersHandle::default();
        let (live, gone) = (Uuid::new_v4(), Uuid::new_v4());
        let (live_tx, mut live_rx) = mpsc::unbounded_channel();
        let (gone_tx, gone_rx) = mpsc::unbounded_channel();
        drop(gone_rx);
        watchers.write().await.insert(live, live_tx);
        watchers.write().await.insert(gone, gone_tx);
        let _actor = tokio::spawn(block_actor_task(
            Add::new(),
            mailbox_rx,
            watchers.clone(),
            gate_rx,
        ));

        // A pin write is a change every watcher is told about.
        let (reply, written) = oneshot::channel();
        mailbox_tx
            .send(BlockMailboxCmd::WriteInput {
                name: "in0".to_string(),
                value: 1.into(),
                reply,
            })
            .await
            .unwrap_or_else(|_| panic!("the actor is receiving"));
        timeout(Duration::from_secs(2), written)
            .await
            .expect("answered")
            .expect("replied")
            .expect("written");
        timeout(Duration::from_secs(2), live_rx.recv())
            .await
            .expect("the live watcher is notified");

        // The actor drops the watcher just after notifying the live one,
        // on another worker.
        for _ in 0..100 {
            if watchers.read().await.len() == 1 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            watchers.read().await.keys().copied().collect::<Vec<_>>(),
            vec![live],
            "only the live watcher is kept"
        );
    }

    /// Regression: see the single-threaded
    /// `pausing_with_a_command_queued_keeps_the_fault`.
    #[tokio::test(flavor = "current_thread")]
    async fn pausing_with_a_command_queued_keeps_the_fault() {
        a_queued_command_keeps_the_fault(true).await;
    }

    /// Regression: see the single-threaded `a_command_keeps_the_fault`.
    #[tokio::test(flavor = "current_thread")]
    async fn a_command_keeps_the_fault() {
        a_queued_command_keeps_the_fault(false).await;
    }

    /// See the single-threaded `a_queued_command_keeps_the_fault`. A
    /// current-thread runtime runs the actor only while the test awaits,
    /// so the gate flip and the queued command reach it together.
    async fn a_queued_command_keeps_the_fault(pause: bool) {
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let (gate, gate_rx) = watch::channel(true);
        let mut block = FaultOnce::new();
        let mut downstream = InputImpl::new("in", HaystackKind::Number, Uuid::new_v4());
        connect_output(&mut block.out, &mut downstream).expect("linked");
        let _actor = tokio::spawn(block_actor_task(
            block,
            mailbox_rx,
            Default::default(),
            gate_rx,
        ));

        // The block faults, its output tells downstream, and the actor
        // recovers optimistically into a cycle that never completes.
        sleep(Duration::from_millis(50)).await;
        assert_matches!(downstream.try_take(), Some((_, Status::Fault)));

        // Queue an inspection — after pausing, if asked — before the
        // actor runs again, so it wakes to both at once.
        if pause {
            gate.send(false).expect("the actor holds the gate");
        }
        for _ in 0..2 {
            let inspected = queue_inspect(&mailbox_tx);
            let state = timeout(Duration::from_secs(2), inspected).await;
            let state = state.expect("answered").expect("replied");
            assert_eq!(state.state, "fault");
            assert_eq!(state.fault_reason.as_deref(), Some(REASON));
            sleep(Duration::from_millis(50)).await;
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
