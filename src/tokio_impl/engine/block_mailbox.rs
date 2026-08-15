// Copyright (c) 2022-2026, Radu Racariu.

//! Per-block mailbox protocol.
//!
//! Defines [`BlockMailboxCmd`] — every external operation against a running
//! block is encoded as one of these commands and sent through that block's
//! `mpsc::Sender<BlockMailboxCmd>`. The receiving actor task handles the
//! command via [`handle_cmd`], which holds `&mut B` (the only mutable borrow
//! of the block, period — no aliasing).

use libhaystack::val::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::base::{
    Status,
    block::{Block, BlockProps, BlockState},
    engine::messages::{BlockDefinition, BlockInputData, BlockOutputData},
    error::EngineError,
    link::{BaseLink, LinkState},
    program::data::{BlockData, LinkData},
};
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Commands sent from the engine to a per-block actor task.
pub(super) enum BlockMailboxCmd {
    /// Reads the block's full inspection snapshot.
    Inspect {
        reply: oneshot::Sender<BlockDefinition>,
    },
    /// Sets an input value. Returns the previous value (if any).
    WriteInput {
        name: String,
        value: Value,
        reply: oneshot::Sender<Result<Option<Value>, String>>,
    },
    /// Sets an output value. Returns the previous value.
    WriteOutput {
        name: String,
        value: Value,
        reply: oneshot::Sender<Result<Value, String>>,
    },
    /// Clone the writer for one of this block's inputs. Used by the engine
    /// when wiring a link: the source block adds a link whose `tx` is the
    /// writer of the target input.
    GetInputWriter {
        name: String,
        reply: oneshot::Sender<Result<WriterImpl, String>>,
    },
    /// Gets the current cached value of one of this block's inputs.
    GetInputValue {
        name: String,
        reply: oneshot::Sender<Option<Value>>,
    },
    /// Gets the current value of one of this block's outputs.
    GetOutputValue {
        name: String,
        reply: oneshot::Sender<Option<Value>>,
    },
    /// Whether this block has an output with the given name.
    HasOutput {
        name: String,
        reply: oneshot::Sender<bool>,
    },
    /// Adds a link from one of this block's outputs to a target input.
    AddOutputLink {
        output_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        target_writer: WriterImpl,
        reply: oneshot::Sender<Result<Uuid, String>>,
    },
    /// Adds a link from one of this block's inputs (chained as a source) to
    /// a target input.
    AddInputLink {
        input_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        target_writer: WriterImpl,
        reply: oneshot::Sender<Result<Uuid, String>>,
    },
    /// Push a value directly into the named input's writer. Used to seed a
    /// freshly linked input with the source's current value.
    SeedInputValue { name: String, value: Value },
    /// Re-send the named input's current cached value through its own
    /// writer. Used by `reset_connected_inputs`.
    RefreshInput { name: String },
    /// Increment the connection count of one of this block's inputs. Called
    /// by the engine after wiring a new link on the source side, so that
    /// `drain_ready_inputs` actually reads from the now-connected input.
    IncrementInput {
        name: String,
        reply: oneshot::Sender<Option<usize>>,
    },
    /// Decrement the connection count of one of this block's inputs.
    /// Returns the new count, or None if the input doesn't exist.
    DecrementInput {
        name: String,
        reply: oneshot::Sender<Option<usize>>,
    },
    /// Disconnect a single link by id. Returns the targets whose
    /// connection counts the engine must decrement.
    DisconnectLink {
        link_id: Uuid,
        reply: oneshot::Sender<Vec<(Uuid, String)>>,
    },
    /// Disconnect ALL of this block's links. Returns the targets whose
    /// connection counts the engine must decrement.
    DisconnectAll {
        reply: oneshot::Sender<Vec<(Uuid, String)>>,
    },
    /// Removes any links from this block's outputs/inputs that target the
    /// given block id.
    RemoveTargetBlockLinks {
        target_block_id: Uuid,
        reply: oneshot::Sender<()>,
    },
    /// Snapshot of this block + its outgoing links for program serialization.
    GetBlockData {
        reply: oneshot::Sender<(BlockData, Vec<LinkData>)>,
    },
    /// Force the block into the Terminated state and exit the actor task.
    Terminate,
}

/// Mailbox capacity per block. 64 is more than enough for a UI session;
/// engine commands are infrequent compared to the per-cycle polling.
pub(super) const BLOCK_MAILBOX_CAP: usize = 64;

/// Send a command that carries a `reply` channel and await the answer.
///
/// Both halves of the round-trip can fail independently — the actor task
/// may be gone before the command is delivered, or it may drop the reply
/// channel after receiving it — and each is reported as a distinct
/// [`EngineError`] naming `block_id`. Every engine round-trip goes
/// through here so those two failure modes are never conflated or
/// reported without the block they refer to.
pub(super) async fn mailbox_request<R>(
    mailbox: &mpsc::Sender<BlockMailboxCmd>,
    block_id: Uuid,
    make_cmd: impl FnOnce(oneshot::Sender<R>) -> BlockMailboxCmd,
) -> Result<R, EngineError> {
    let (reply, response) = oneshot::channel();

    mailbox_send(mailbox, block_id, make_cmd(reply)).await?;

    response
        .await
        .map_err(|_| EngineError::BlockDroppedReply { id: block_id })
}

/// Send a command that has no `reply` channel.
pub(super) async fn mailbox_send(
    mailbox: &mpsc::Sender<BlockMailboxCmd>,
    block_id: Uuid,
    cmd: BlockMailboxCmd,
) -> Result<(), EngineError> {
    mailbox
        .send(cmd)
        .await
        .map_err(|_| EngineError::BlockTaskGone { id: block_id })
}

/// Handle a single mailbox command against a block. Returns `true` if the
/// command was [`BlockMailboxCmd::Terminate`] (signalling the actor task to
/// exit its loop).
pub(super) async fn handle_cmd<B>(cmd: BlockMailboxCmd, block: &mut B) -> bool
where
    B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
{
    match cmd {
        BlockMailboxCmd::Inspect { reply } => {
            let _ = reply.send(snapshot_block_definition(block));
        }

        BlockMailboxCmd::WriteInput { name, value, reply } => {
            let result = match block.get_input_mut(&name) {
                Some(input) => {
                    let prev = input.get_value().cloned();
                    // External writes are always Ok status — they come from
                    // the engine, not from a producer block.
                    input.set_value(value, Status::Ok);
                    Ok(prev)
                }
                None => Err("Input not found".to_string()),
            };
            let _ = reply.send(result);
        }

        BlockMailboxCmd::WriteOutput { name, value, reply } => {
            let result = match block.get_output_mut(&name) {
                Some(output) => {
                    let prev = output.value().clone();
                    output.set(value);
                    Ok(prev)
                }
                None => Err("Output not found".to_string()),
            };
            let _ = reply.send(result);
        }

        BlockMailboxCmd::GetInputWriter { name, reply } => {
            let result = match block.get_input_mut(&name) {
                Some(input) => Ok(input.writer().clone()),
                None => Err("Input not found".to_string()),
            };
            let _ = reply.send(result);
        }

        BlockMailboxCmd::GetInputValue { name, reply } => {
            let value = block
                .get_input(&name)
                .and_then(|input| input.get_value().cloned());
            let _ = reply.send(value);
        }

        BlockMailboxCmd::GetOutputValue { name, reply } => {
            let value = block.get_output(&name).map(|output| output.value().clone());
            let _ = reply.send(value);
        }

        BlockMailboxCmd::HasOutput { name, reply } => {
            let _ = reply.send(block.get_output(&name).is_some());
        }

        BlockMailboxCmd::AddOutputLink {
            output_name,
            target_block_id,
            target_input_name,
            target_writer,
            reply,
        } => {
            let result = add_output_link_inner(
                block,
                &output_name,
                target_block_id,
                target_input_name,
                target_writer,
            );
            let _ = reply.send(result);
        }

        BlockMailboxCmd::AddInputLink {
            input_name,
            target_block_id,
            target_input_name,
            target_writer,
            reply,
        } => {
            let result = add_input_link_inner(
                block,
                &input_name,
                target_block_id,
                target_input_name,
                target_writer,
            );
            let _ = reply.send(result);
        }

        BlockMailboxCmd::SeedInputValue { name, value } => {
            if let Some(input) = block.get_input_mut(&name) {
                let _ = input.writer().send((value, Status::Ok));
            }
        }

        BlockMailboxCmd::RefreshInput { name } => {
            if let Some(input) = block.get_input_mut(&name) {
                let value = input.get_value().cloned().unwrap_or_default();
                let status = input.status();
                let _ = input.writer().send((value, status));
            }
        }

        BlockMailboxCmd::IncrementInput { name, reply } => {
            let count = block
                .get_input_mut(&name)
                .map(|input| input.increment_conn());
            let _ = reply.send(count);
        }

        BlockMailboxCmd::DecrementInput { name, reply } => {
            let count = block
                .get_input_mut(&name)
                .map(|input| input.decrement_conn());
            let _ = reply.send(count);
        }

        BlockMailboxCmd::DisconnectLink { link_id, reply } => {
            let targets = collect_targets_for_link(block, &link_id);
            block.remove_link_by_id(&link_id);
            let _ = reply.send(targets);
        }

        BlockMailboxCmd::DisconnectAll { reply } => {
            let targets = collect_all_targets(block);
            block.remove_all_links();
            let _ = reply.send(targets);
        }

        BlockMailboxCmd::RemoveTargetBlockLinks {
            target_block_id,
            reply,
        } => {
            for output in block.outputs_mut().iter_mut() {
                output.remove_target_block_links(&target_block_id);
            }
            for input in block.inputs_mut().iter_mut() {
                input.remove_target_block_links(&target_block_id);
            }
            let _ = reply.send(());
        }

        BlockMailboxCmd::GetBlockData { reply } => {
            let _ = reply.send(snapshot_block_data(block));
        }

        BlockMailboxCmd::Terminate => {
            block.set_state(BlockState::Terminated);
            return true;
        }
    }

    false
}

fn snapshot_block_definition<B: BlockProps + ?Sized>(block: &B) -> BlockDefinition {
    let state = block.state();
    BlockDefinition {
        id: block.id().to_string(),
        name: block.name().to_string(),
        library: block.desc().library.clone(),
        inputs: block
            .inputs()
            .iter()
            .map(|input| {
                (
                    input.name().to_string(),
                    BlockInputData {
                        kind: input.kind().to_string(),
                        val: input.get_value().cloned().unwrap_or_default(),
                        is_connected: input.is_connected(),
                    },
                )
            })
            .collect(),
        outputs: block
            .outputs()
            .iter()
            .map(|output| {
                (
                    output.desc().name.to_string(),
                    BlockOutputData {
                        kind: output.desc().kind.to_string(),
                        val: output.value().clone(),
                    },
                )
            })
            .collect(),
        fault_reason: state.fault_reason().map(|s| s.to_string()),
        state: state.label().to_string(),
    }
}

fn snapshot_block_data<B: BlockProps + ?Sized>(block: &B) -> (BlockData, Vec<LinkData>) {
    let block_data = BlockData {
        id: block.id().to_string(),
        name: block.name().to_string(),
        dis: block.desc().dis.to_string(),
        lib: block.desc().library.clone(),
        category: block.desc().category.clone(),
        ver: block.desc().ver.clone(),
    };

    let mut links = Vec::new();
    for (pin_name, pin_links) in block.links() {
        for link in pin_links {
            links.push(LinkData {
                id: Some(link.id().to_string()),
                source_block_pin_name: pin_name.to_string(),
                source_block_uuid: block.id().to_string(),
                target_block_pin_name: link.target_input().to_string(),
                target_block_uuid: link.target_block_id().to_string(),
            });
        }
    }
    (block_data, links)
}

fn collect_targets_for_link<B: BlockProps + ?Sized>(
    block: &mut B,
    link_id: &Uuid,
) -> Vec<(Uuid, String)> {
    let mut targets = Vec::new();
    for output in block.outputs_mut().iter() {
        for link in output.links() {
            if link.id() == link_id {
                targets.push((*link.target_block_id(), link.target_input().to_string()));
            }
        }
    }
    for input in block.inputs_mut().iter() {
        for link in input.links() {
            if link.id() == link_id {
                targets.push((*link.target_block_id(), link.target_input().to_string()));
            }
        }
    }
    targets
}

fn collect_all_targets<B: BlockProps + ?Sized>(block: &mut B) -> Vec<(Uuid, String)> {
    let mut targets = Vec::new();
    for output in block.outputs_mut().iter().filter(|o| o.is_connected()) {
        for link in output.links() {
            targets.push((*link.target_block_id(), link.target_input().to_string()));
        }
    }
    for input in block.inputs_mut().iter().filter(|i| i.has_output()) {
        for link in input.links() {
            targets.push((*link.target_block_id(), link.target_input().to_string()));
        }
    }
    targets
}

fn add_output_link_inner<B: Block<Writer = WriterImpl, Reader = ReaderImpl> + ?Sized>(
    block: &mut B,
    output_name: &str,
    target_block_id: Uuid,
    target_input_name: String,
    target_writer: WriterImpl,
) -> Result<Uuid, String> {
    let mut outputs = block.outputs_mut();
    let output = outputs
        .iter_mut()
        .find(|o| o.desc().name == output_name)
        .ok_or_else(|| "Output not found".to_string())?;

    if output.links().iter().any(|link| {
        link.target_block_id() == &target_block_id && link.target_input() == target_input_name
    }) {
        return Err("Already connected".to_string());
    }

    let mut link = BaseLink::new(target_block_id, target_input_name);
    let id = link.id;
    link.tx = Some(target_writer);
    link.state = LinkState::Connected;

    output.add_link(link);
    Ok(id)
}

fn add_input_link_inner<B: Block<Writer = WriterImpl, Reader = ReaderImpl> + ?Sized>(
    block: &mut B,
    input_name: &str,
    target_block_id: Uuid,
    target_input_name: String,
    target_writer: WriterImpl,
) -> Result<Uuid, String> {
    let block_id = *block.id();
    if block_id == target_block_id {
        return Err("Cannot connect to the same block".to_string());
    }

    let mut inputs = block.inputs_mut();
    let input = inputs
        .iter_mut()
        .find(|i| i.name() == input_name)
        .ok_or_else(|| "Input not found".to_string())?;

    if input.links().iter().any(|link| {
        link.target_block_id() == &target_block_id && link.target_input() == target_input_name
    }) {
        return Err("Already connected".to_string());
    }

    let mut link = BaseLink::new(target_block_id, target_input_name);
    let id = link.id;
    link.tx = Some(target_writer);
    link.state = LinkState::Connected;

    input.add_link(link);
    Ok(id)
}
