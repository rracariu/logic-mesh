// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block actor task for the multi-threaded engine.
//!
//! Same shape as the single-threaded variant: the actor task **owns the
//! block by value** and [`tokio::select!`]s between `block.execute()` and a
//! per-block mailbox. The MT-specific differences are:
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

use super::super::block_mailbox::{BlockMailboxCmd, handle_cmd};
use crate::base::Status;
use crate::base::block::{Block, BlockState};
use crate::base::engine::messages::{ChangeSource, WatchMessage};
use crate::tokio_impl::MtBlock;

/// MT-side watchers handle: cross-thread, async-locked.
pub(super) type WatchersHandle = Arc<RwLock<BTreeMap<Uuid, UnboundedSender<WatchMessage>>>>;

/// Per-block actor task. Owns the block by value; processes mailbox
/// commands interleaved with `block.execute()` cycles.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
) where
    B: MtBlock + 'static,
{
    let mut last_pin_values = BTreeMap::<String, Value>::new();
    let mut last_state = BlockState::Running;
    let mut terminated = false;

    while !terminated {
        // Optimistic recovery — see single_threaded::actor for rationale.
        if block.state().is_fault() {
            block.set_state(BlockState::Running);
        }

        terminated = run_one_step(&mut block, &mut mailbox).await;

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

        if matches!(block.state(), BlockState::Terminated) {
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

/// Drive the block one step. Returns `true` if the actor task should exit.
async fn run_one_step<B>(block: &mut B, mailbox: &mut mpsc::Receiver<BlockMailboxCmd>) -> bool
where
    B: MtBlock + 'static,
{
    // See single_threaded::actor::run_one_step for the encoding.
    let mut cmd_to_handle: Option<Option<BlockMailboxCmd>> = None;
    {
        let execute_fut = block.execute();
        tokio::pin!(execute_fut);
        tokio::select! {
            biased;
            cmd = mailbox.recv() => {
                cmd_to_handle = Some(cmd);
            }
            () = &mut execute_fut => {}
        }
    }

    match cmd_to_handle {
        None => false,
        Some(Some(cmd)) => handle_cmd(cmd, block).await,
        // Closed mailbox: exit — see single_threaded::actor::run_one_step.
        Some(None) => {
            block.set_state(BlockState::Terminated);
            true
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
        // Unbounded channel — see single_threaded/actor.rs rationale.
        for sender in notification_channels.read().await.values() {
            let _ = sender.send(WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
                state: state.clone(),
            });
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::{sync::mpsc, time::timeout};

    use super::block_actor_task;
    use crate::blocks::math::Add;
    use crate::tokio_impl::engine::block_mailbox::BLOCK_MAILBOX_CAP;

    /// Regression: see the single-threaded
    /// `actor_exits_when_its_mailbox_closes`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn actor_exits_when_its_mailbox_closes() {
        let (mailbox_tx, mailbox_rx) = mpsc::channel(BLOCK_MAILBOX_CAP);
        let task = tokio::spawn(block_actor_task(Add::new(), mailbox_rx, Default::default()));

        drop(mailbox_tx);

        timeout(Duration::from_secs(2), task)
            .await
            .expect("actor task exits once its mailbox closes")
            .expect("actor task completes without panicking");
    }
}
