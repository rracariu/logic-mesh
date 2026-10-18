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
    block::{
        Block, BlockProps, BlockState,
        connect::{ensure_not_linked, ensure_not_self_link, input_not_found, output_not_found},
    },
    engine::messages::{BlockDefinition, BlockInputData, BlockOutputData},
    error::{ConnectError, EngineError},
    link::BaseLink,
    program::data::{BlockData, LinkData},
};
use crate::tokio_impl::block::write_input_value;
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
        reply: oneshot::Sender<Result<Option<Value>, EngineError>>,
    },
    /// Sets an output value. Returns the previous value.
    WriteOutput {
        name: String,
        value: Value,
        reply: oneshot::Sender<Result<Value, EngineError>>,
    },
    /// Clone the writer for one of this block's inputs. Used by the engine
    /// when wiring a link: the source block adds a link whose `tx` is the
    /// writer of the target input.
    GetInputWriter {
        name: String,
        reply: oneshot::Sender<Result<WriterImpl, EngineError>>,
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
    /// Adds link `link_id` from one of this block's outputs to a target
    /// input.
    AddOutputLink {
        link_id: Uuid,
        output_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        target_writer: WriterImpl,
        reply: oneshot::Sender<Result<(), ConnectError>>,
    },
    /// Adds link `link_id` from one of this block's inputs (chained as a
    /// source) to a target input.
    AddInputLink {
        link_id: Uuid,
        input_name: String,
        target_block_id: Uuid,
        target_input_name: String,
        target_writer: WriterImpl,
        reply: oneshot::Sender<Result<(), ConnectError>>,
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

/// Asks the actor behind `mailbox` — a removed block's last sender — to
/// terminate, without waiting on the actor.
///
/// Never blocks: when the mailbox is full, the [`BlockMailboxCmd::Terminate`]
/// is skipped, and dropping `mailbox` closes it instead — the actor then
/// exits once it has drained the commands already queued. An awaited send
/// could wait forever on a single-threaded engine, whose actors only make
/// progress while the engine drives its `LocalSet`.
pub(super) fn terminate_actor(mailbox: mpsc::Sender<BlockMailboxCmd>) {
    let _ = mailbox.try_send(BlockMailboxCmd::Terminate);
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
                    write_input_value(input, value);
                    Ok(prev)
                }
                None => Err(EngineError::InputNotFound {
                    block: *block.id(),
                    pin: name,
                }),
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
                None => Err(EngineError::OutputNotFound {
                    block: *block.id(),
                    pin: name,
                }),
            };
            let _ = reply.send(result);
        }

        BlockMailboxCmd::GetInputWriter { name, reply } => {
            let result = match block.get_input_mut(&name) {
                Some(input) => Ok(input.writer().clone()),
                None => Err(EngineError::InputNotFound {
                    block: *block.id(),
                    pin: name,
                }),
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
            link_id,
            output_name,
            target_block_id,
            target_input_name,
            target_writer,
            reply,
        } => {
            let result = add_output_link_inner(
                block,
                link_id,
                &output_name,
                target_block_id,
                target_input_name,
                target_writer,
            );
            let _ = reply.send(result);
        }

        BlockMailboxCmd::AddInputLink {
            link_id,
            input_name,
            target_block_id,
            target_input_name,
            target_writer,
            reply,
        } => {
            let result = add_input_link_inner(
                block,
                link_id,
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
    let mut definition = BlockDefinition::new(
        block.id().to_string(),
        block.name(),
        block.desc().library.clone(),
    )
    .with_state(&block.state());
    for input in block.inputs() {
        definition = definition.with_input(
            input.name(),
            BlockInputData::new(
                input.kind().to_string(),
                input.get_value().cloned().unwrap_or_default(),
                input.is_connected(),
            ),
        );
    }
    for output in block.outputs() {
        definition = definition.with_output(
            output.desc().name.clone(),
            BlockOutputData::new(output.desc().kind.to_string(), output.value().clone()),
        );
    }
    definition
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
    link_id: Uuid,
    output_name: &str,
    target_block_id: Uuid,
    target_input_name: String,
    target_writer: WriterImpl,
) -> Result<(), ConnectError> {
    let block_id = *block.id();
    let mut outputs = block.outputs_mut();
    let output = outputs
        .iter_mut()
        .find(|o| o.desc().name == output_name)
        .ok_or_else(|| output_not_found(block_id, output_name))?;

    ensure_not_linked(
        &output.links(),
        (&block_id, output_name),
        (&target_block_id, &target_input_name),
    )?;

    output.add_link(BaseLink::connected(
        link_id,
        target_block_id,
        target_input_name,
        target_writer,
    ));
    Ok(())
}

fn add_input_link_inner<B: Block<Writer = WriterImpl, Reader = ReaderImpl> + ?Sized>(
    block: &mut B,
    link_id: Uuid,
    input_name: &str,
    target_block_id: Uuid,
    target_input_name: String,
    target_writer: WriterImpl,
) -> Result<(), ConnectError> {
    let block_id = *block.id();
    let source = (&block_id, input_name);
    let target = (&target_block_id, target_input_name.as_str());
    ensure_not_self_link(source, target)?;

    let mut inputs = block.inputs_mut();
    let input = inputs
        .iter_mut()
        .find(|i| i.name() == input_name)
        .ok_or_else(|| input_not_found(block_id, input_name))?;

    ensure_not_linked(&input.links(), source, target)?;

    input.add_link(BaseLink::connected(
        link_id,
        target_block_id,
        target_input_name,
        target_writer,
    ));
    Ok(())
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use super::{BlockMailboxCmd, handle_cmd};
    use crate::base::block::BlockProps;
    use crate::base::error::{ConnectError, EngineError};
    use crate::blocks::math::Add;

    /// A block rejecting a command answers with what it rejected, typed:
    /// the pin it does not have, or the link it cannot take.
    #[tokio::test]
    async fn rejections_are_typed() {
        let mut add = Add::new();
        let id = *add.id();

        let (reply, response) = oneshot::channel();
        let cmd = BlockMailboxCmd::WriteInput {
            name: "nope".to_string(),
            value: 1.into(),
            reply,
        };
        handle_cmd(cmd, &mut add).await;
        assert_matches!(
            response.await.expect("answered"),
            Err(EngineError::InputNotFound { block, pin }) if block == id && pin == "nope"
        );

        let (reply, response) = oneshot::channel();
        let target_writer = add.inputs_mut()[1].writer().clone();
        let cmd = BlockMailboxCmd::AddInputLink {
            link_id: Uuid::new_v4(),
            input_name: "in0".to_string(),
            target_block_id: id,
            target_input_name: "in1".to_string(),
            target_writer,
            reply,
        };
        handle_cmd(cmd, &mut add).await;
        assert_matches!(
            response.await.expect("answered"),
            Err(ConnectError::SelfLink { block, source_input, target_input })
                if block == id && source_input == "in0" && target_input == "in1"
        );
    }
}
