// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block actor task for the multi-threaded engine.
//!
//! Same shape as the single-threaded variant: the actor task **owns the
//! block by value** and `tokio::select!`s between `block.execute()` and a
//! per-block mailbox. The MT-specific differences are:
//!
//! - The block is sent across thread boundaries (worker thread) once, then
//!   stays put. `B: Send` is required at scheduling time, but the actor
//!   future itself is `!Send` (runs on the worker's `LocalSet`).
//! - Watchers are stored as `Arc<RwLock<...>>` (cross-worker visibility),
//!   so the change-of-value check is async.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, Sender},
};
use uuid::Uuid;

use super::super::block_mailbox::{BlockMailboxCmd, handle_cmd};
use crate::base::Status;
use crate::base::block::{Block, BlockState};
use crate::base::engine::messages::{ChangeSource, WatchMessage};
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// MT-side watchers handle: cross-thread, async-locked.
pub(super) type WatchersHandle = Arc<RwLock<BTreeMap<Uuid, Sender<WatchMessage>>>>;

/// Per-block actor task. Owns the block by value; processes mailbox
/// commands interleaved with `block.execute()` cycles.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
) where
    B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
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
    B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
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
    B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
{
    let mut cmd_to_handle: Option<BlockMailboxCmd> = None;
    {
        let execute_fut = block.execute();
        tokio::pin!(execute_fut);
        tokio::select! {
            biased;
            cmd = mailbox.recv() => {
                cmd_to_handle = cmd;
            }
            () = &mut execute_fut => {}
        }
    }

    let Some(cmd) = cmd_to_handle else {
        return false;
    };

    handle_cmd(cmd, block).await
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
        for sender in notification_channels.read().await.values() {
            let _ = sender.try_send(WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
                state: state.clone(),
            });
        }
    }
}
