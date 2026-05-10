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

use std::collections::{BTreeMap, HashMap};

use libhaystack::val::Value;
use tokio::sync::mpsc;

use super::mailbox::{BlockMailboxCmd, WatchersHandle, handle_cmd};
use crate::base::{
    block::{Block, BlockState},
    engine::messages::ChangeSource,
};
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Per-block actor task. Owns the block by value; processes mailbox commands
/// interleaved with `block.execute()` cycles.
pub(super) async fn block_actor_task<B>(
    mut block: B,
    mut mailbox: mpsc::Receiver<BlockMailboxCmd>,
    watchers: WatchersHandle,
) where
    B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
{
    let mut last_pin_values = BTreeMap::<String, Value>::new();
    let mut terminated = false;

    while !terminated {
        // Drive one step: either execute completes, or a mailbox cmd arrives
        // (cancelling execute mid-await).
        terminated = run_one_step(&mut block, &mut mailbox).await;

        change_of_value_check(&watchers, &block, &mut last_pin_values);

        if block.state() == BlockState::Terminated {
            break;
        }

        // Co-operative yield: prevents a tight-cycling block (e.g. a
        // misconfigured periodic block with no awaits) from starving siblings
        // on the same LocalSet.
        tokio::task::yield_now().await;
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

/// Detect changes on the block's pins relative to the previously-emitted
/// snapshot and dispatch a `WatchMessage` to every subscribed watcher.
fn change_of_value_check<B: Block + 'static>(
    notification_channels: &WatchersHandle,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
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

    if !changes.is_empty() {
        for sender in notification_channels.borrow().values() {
            let _ = sender.try_send(crate::base::engine::messages::WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
            });
        }
    }
}

