// Copyright (c) 2022-2026, Radu Racariu.

//! The block graph both engines keep, and the running-phase operations
//! their requests are routed to.
//!
//! An engine holds one [`BlockHandle`] per scheduled block. The block
//! itself lives in its actor task (see `block_mailbox`), so the handle
//! carries the block's mailbox and a snapshot of its static identity.
//! Every operation on the graph — inspecting a block, writing a pin,
//! wiring or removing a link, saving or loading a program — is a
//! sequence of mailbox round-trips, the same for either engine: the
//! engines differ only in how they host the actor tasks, which a
//! [`ProgramHost`] abstracts for the one step that creates blocks.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::block_mailbox::{BlockMailboxCmd, mailbox_request, mailbox_send, terminate_actor};
use super::program_block_ids;
use crate::base::block::{BlockDesc, BlockProps};
use crate::base::engine::messages::BlockDefinition;
use crate::base::error::{EngineError, LinkEnd, RegistryError, Result, parse_block_uuid};
use crate::base::program::{
    Program,
    data::{LinkData, PinValue, Position, ProgramBlock},
};
use crate::blocks::registry::get_block;

/// Engine-side handle for a scheduled block.
///
/// Holds the mailbox sender and a cached snapshot of the block's static
/// identity (id / name / lib / desc), so the engine can answer cheap queries
/// without a mailbox round-trip.
pub struct BlockHandle {
    id: Uuid,
    name: String,
    library: String,
    /// Owned clone of the block's descriptor.
    ///
    /// We clone instead of borrowing `&'static` [`BlockDesc`] because for
    /// JS blocks the descriptor lives inside the block instance — and
    /// the instance gets moved into its actor task at schedule time,
    /// invalidating any borrowed pointer. Native (Rust) blocks back
    /// their descriptor with a
    /// [`LazyLock`](std::sync::LazyLock)`<`[`BlockDesc`]`>` so a
    /// `&'static` would be sound for them, but we use a single uniform
    /// owned shape to avoid an unsafe trait split. The clone happens
    /// once per block at schedule time.
    desc: BlockDesc,
    mailbox: mpsc::Sender<BlockMailboxCmd>,
    /// UI display label. Pure passthrough metadata — the engine never
    /// reads it. Stored here so `save_program` can round-trip it without
    /// the UI layer being the canonical store.
    label: Option<String>,
    /// UI position. Same role as `label`: pure passthrough so headless
    /// save/load preserves the layout.
    position: Option<Position>,
}

impl BlockHandle {
    /// Returns the block's unique id.
    pub fn id(&self) -> &Uuid {
        &self.id
    }
    /// Returns the block's type name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the block's library name.
    pub fn library(&self) -> &str {
        &self.library
    }
    /// Returns the block's descriptor.
    pub fn desc(&self) -> &BlockDesc {
        &self.desc
    }
    /// Returns the user-supplied display label, if any.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    /// Returns the UI position, if any.
    pub fn position(&self) -> Option<Position> {
        self.position
    }
}

/// An engine's scheduled blocks, keyed by block id.
#[derive(Default)]
pub(super) struct BlockGraph {
    handles: BTreeMap<Uuid, BlockHandle>,
}

impl BlockGraph {
    /// Every scheduled block's handle.
    pub(super) fn handles(&self) -> Vec<&BlockHandle> {
        self.handles.values().collect()
    }

    /// The handle of block `id`, if scheduled.
    pub(super) fn handle(&self, id: &Uuid) -> Option<&BlockHandle> {
        self.handles.get(id)
    }

    /// Whether block `id` is scheduled.
    pub(super) fn contains(&self, id: &Uuid) -> bool {
        self.handles.contains_key(id)
    }

    /// The mailbox of block `id`'s actor, if scheduled.
    pub(super) fn mailbox(&self, id: &Uuid) -> Option<&mpsc::Sender<BlockMailboxCmd>> {
        self.handles.get(id).map(|h| &h.mailbox)
    }

    /// [`handle`](Self::handle) for callers that treat an unscheduled id
    /// as an error rather than an absence.
    fn handle_or_err(&self, id: &Uuid) -> Result<&BlockHandle, EngineError> {
        self.handle(id)
            .ok_or(EngineError::BlockInstanceNotFound { id: *id })
    }

    /// [`mailbox`](Self::mailbox), same treatment.
    fn mailbox_or_err(&self, id: &Uuid) -> Result<&mpsc::Sender<BlockMailboxCmd>, EngineError> {
        self.mailbox(id)
            .ok_or(EngineError::BlockInstanceNotFound { id: *id })
    }

    /// Records `block`, about to move into the actor task that reads
    /// `mailbox`.
    ///
    /// Rejects an id that is already scheduled: replacing the handle
    /// would orphan the running block, whose links other blocks still
    /// point at.
    pub(super) fn insert<B: BlockProps + ?Sized>(
        &mut self,
        block: &B,
        mailbox: mpsc::Sender<BlockMailboxCmd>,
    ) -> Result<(), EngineError> {
        let id = *block.id();
        if self.handles.contains_key(&id) {
            return Err(EngineError::DuplicateBlockId { id });
        }
        // Clone the desc into the handle BEFORE `block` moves into its
        // actor task. JS blocks' "static" desc actually lives inside the
        // block instance; borrowing it across the move is UB.
        let handle = BlockHandle {
            id,
            name: block.name().to_string(),
            library: block.desc().library.clone(),
            desc: block.desc().clone(),
            mailbox,
            label: None,
            position: None,
        };
        self.handles.insert(id, handle);
        Ok(())
    }

    /// Updates block `id`'s UI metadata: each of `label` and `position`
    /// that is given replaces the stored one.
    pub(super) fn set_metadata(
        &mut self,
        id: &Uuid,
        label: Option<String>,
        position: Option<Position>,
    ) {
        if let Some(handle) = self.handles.get_mut(id) {
            if let Some(label) = label {
                handle.label = Some(label);
            }
            if let Some(position) = position {
                handle.position = Some(position);
            }
        }
    }

    /// Removes every block, telling each actor to terminate.
    ///
    /// The actors are told without waiting on their mailboxes: on a
    /// single-threaded engine no actor makes progress while the engine
    /// resets, so one full mailbox would block an awaited send for good.
    pub(super) fn clear(&mut self) {
        for (_, handle) in std::mem::take(&mut self.handles) {
            terminate_actor(handle.mailbox);
        }
    }

    /// Validates `link_data` against the scheduled blocks and returns it
    /// with an id assigned.
    ///
    /// Wiring requires mailbox round-trips between the source and target
    /// actor tasks, which only answer while the engine runs. This checks
    /// synchronously that both blocks and the named pins exist by
    /// consulting each block's static `BlockDesc`.
    pub(super) fn validate_link(&self, link_data: &LinkData) -> Result<LinkData> {
        let source_id = parse_block_uuid(&link_data.source_block_uuid)?;
        let target_id = parse_block_uuid(&link_data.target_block_uuid)?;
        let source_handle = self.handle_or_err(&source_id)?;
        let target_handle = self.handle_or_err(&target_id)?;

        let source_pin = link_data.source_block_pin_name.as_str();
        let source_pin_exists = source_handle
            .desc()
            .outputs
            .iter()
            .any(|o| o.name == source_pin)
            || source_handle
                .desc()
                .inputs
                .iter()
                .any(|i| i.name == source_pin);
        if !source_pin_exists {
            return Err(EngineError::PinNotFound {
                end: LinkEnd::Source,
                block: source_id,
                pin: source_pin.to_string(),
            }
            .into());
        }

        let target_pin = link_data.target_block_pin_name.as_str();
        let target_pin_exists = target_handle
            .desc()
            .inputs
            .iter()
            .any(|i| i.name == target_pin);
        if !target_pin_exists {
            return Err(EngineError::PinNotFound {
                end: LinkEnd::Target,
                block: target_id,
                pin: target_pin.to_string(),
            }
            .into());
        }

        let id = link_data
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        Ok(LinkData {
            id: Some(id),
            ..link_data.clone()
        })
    }

    /// Reads block `id`'s full inspection snapshot.
    pub(super) async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, EngineError> {
        let mailbox = self.mailbox_or_err(id)?;

        mailbox_request(mailbox, *id, |reply| BlockMailboxCmd::Inspect { reply }).await
    }

    /// Writes `value` to block `id`'s input `name`, returning the
    /// previous value.
    pub(super) async fn write_input(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Option<Value>, EngineError> {
        let mailbox = self.mailbox_or_err(id)?;

        mailbox_request(mailbox, *id, |reply| BlockMailboxCmd::WriteInput {
            name,
            value,
            reply,
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)
    }

    /// Writes `value` to block `id`'s output `name`, returning the
    /// previous value.
    pub(super) async fn write_output(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Value, EngineError> {
        let mailbox = self.mailbox_or_err(id)?;

        mailbox_request(mailbox, *id, |reply| BlockMailboxCmd::WriteOutput {
            name,
            value,
            reply,
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)
    }

    /// Connects two blocks (source pin → target input). The source pin can
    /// be either an output or an input (the latter is input-fanout).
    pub(super) async fn connect_blocks(&self, link_data: &LinkData) -> Result<LinkData> {
        let source_id = parse_block_uuid(&link_data.source_block_uuid)?;
        let target_id = parse_block_uuid(&link_data.target_block_uuid)?;

        let source_mb = self.mailbox_or_err(&source_id)?;
        let target_mb = self.mailbox_or_err(&target_id)?;

        // Get the writer of the target input.
        let target_writer = mailbox_request(target_mb, target_id, |reply| {
            BlockMailboxCmd::GetInputWriter {
                name: link_data.target_block_pin_name.clone(),
                reply,
            }
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)?;

        // Try AddOutputLink first; if the source pin is not an output, fall
        // back to AddInputLink.
        let is_output = mailbox_request(source_mb, source_id, |reply| BlockMailboxCmd::HasOutput {
            name: link_data.source_block_pin_name.clone(),
            reply,
        })
        .await?;

        let link_id = mailbox_request(source_mb, source_id, |reply| {
            if is_output {
                BlockMailboxCmd::AddOutputLink {
                    output_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_id,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    target_writer: target_writer.clone(),
                    reply,
                }
            } else {
                BlockMailboxCmd::AddInputLink {
                    input_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_id,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    target_writer: target_writer.clone(),
                    reply,
                }
            }
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)?;

        // Increment the target's connection count for this input. Without
        // this, the input would read as unconnected — no input wait wakes
        // on it — and `is_connected` would misreport it. The new count
        // itself is of no interest, so only the delivery half of the
        // round-trip is checked.
        let (inc_reply, inc_response) = oneshot::channel();
        mailbox_send(
            target_mb,
            target_id,
            BlockMailboxCmd::IncrementInput {
                name: link_data.target_block_pin_name.clone(),
                reply: inc_reply,
            },
        )
        .await?;
        let _ = inc_response.await;

        // Seed the target input with the source's current value so the
        // newly-connected block sees a value immediately.
        let seed_value = self
            .read_source_value(&source_id, &link_data.source_block_pin_name, is_output)
            .await
            .unwrap_or_default();
        mailbox_send(
            target_mb,
            target_id,
            BlockMailboxCmd::SeedInputValue {
                name: link_data.target_block_pin_name.clone(),
                value: seed_value,
            },
        )
        .await?;
        // Refresh another connected input on the target so the block
        // re-cycles with the freshly-seeded value.
        self.reset_connected_inputs(&target_id, &link_data.target_block_pin_name)
            .await;

        Ok(LinkData {
            id: Some(link_id.to_string()),
            ..link_data.clone()
        })
    }

    /// The current value of the source pin of a link: an output's value
    /// or, for input fan-out, the input's cached value.
    async fn read_source_value(
        &self,
        source_id: &Uuid,
        source_pin: &str,
        is_output: bool,
    ) -> Option<Value> {
        let mb = self.mailbox(source_id)?;
        let (tx, rx) = oneshot::channel();
        let cmd = if is_output {
            BlockMailboxCmd::GetOutputValue {
                name: source_pin.to_string(),
                reply: tx,
            }
        } else {
            BlockMailboxCmd::GetInputValue {
                name: source_pin.to_string(),
                reply: tx,
            }
        };
        mb.send(cmd).await.ok()?;
        rx.await.ok().flatten()
    }

    /// Re-arms any other connected input on the target block by
    /// re-sending its cached value, so the block re-cycles after a new
    /// link is added.
    ///
    /// Skips unconnected inputs: their cached value would not re-run the
    /// block on its own, and the mailbox round-trip would be wasted.
    async fn reset_connected_inputs(&self, target_id: &Uuid, ignore_input: &str) {
        let Ok(definition) = self.inspect_block(target_id).await else {
            return;
        };
        if let Some((name, _data)) = definition
            .inputs
            .iter()
            .find(|(name, data)| name.as_str() != ignore_input && data.is_connected)
            && let Some(mb) = self.mailbox(target_id)
        {
            let _ = mb
                .send(BlockMailboxCmd::RefreshInput { name: name.clone() })
                .await;
        }
    }

    /// Removes block `block_id` and all its links.
    pub(super) async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        // 1. Tell the target to disconnect itself; collect targets that need
        //    decrementing.
        let target_mb = self.mailbox_or_err(block_id)?.clone();
        let targets = mailbox_request(&target_mb, *block_id, |reply| {
            BlockMailboxCmd::DisconnectAll { reply }
        })
        .await?;

        for (other_id, input_name) in targets {
            self.decrement_input(&other_id, input_name).await;
        }

        // 2. Broadcast RemoveTargetBlockLinks to every OTHER block.
        let other_ids: Vec<Uuid> = self
            .handles
            .keys()
            .copied()
            .filter(|id| id != block_id)
            .collect();
        for other_id in other_ids {
            if let Some(mb) = self.mailbox(&other_id) {
                let (tx, rx) = oneshot::channel();
                let _ = mb
                    .send(BlockMailboxCmd::RemoveTargetBlockLinks {
                        target_block_id: *block_id,
                        reply: tx,
                    })
                    .await;
                let _ = rx.await;
            }
        }

        // 3. Terminate the block.
        let _ = target_mb.send(BlockMailboxCmd::Terminate).await;
        self.handles.remove(block_id);

        Ok(*block_id)
    }

    /// Decrements the connection count of block `id`'s input `name`, for
    /// a link to it that is gone. Best effort: a block that is gone
    /// itself has nothing left to count.
    async fn decrement_input(&self, id: &Uuid, name: String) {
        if let Some(mb) = self.mailbox(id) {
            let _ = mailbox_request(mb, *id, |reply| BlockMailboxCmd::DecrementInput {
                name,
                reply,
            })
            .await;
        }
    }

    /// Snapshot the full program: every scheduled block with its UI
    /// metadata + current pin values, plus every link. This is the
    /// canonical save format — round-trips through a program load.
    pub(super) async fn save_program(&self) -> Result<Program> {
        let mut blocks = BTreeMap::new();
        let mut links = BTreeMap::new();

        for (id, handle) in &self.handles {
            // Pull dynamic state (pin values + per-input connectedness)
            // via `Inspect`.
            let definition = mailbox_request(&handle.mailbox, *id, |reply| {
                BlockMailboxCmd::Inspect { reply }
            })
            .await?;

            // Pull outgoing links via `GetBlockData`.
            let (_block_data, block_links) = mailbox_request(&handle.mailbox, *id, |reply| {
                BlockMailboxCmd::GetBlockData { reply }
            })
            .await?;

            let inputs = definition
                .inputs
                .into_iter()
                .map(|(name, data)| {
                    (
                        name,
                        PinValue {
                            value: data.val,
                            is_connected: data.is_connected,
                        },
                    )
                })
                .collect();
            let outputs = definition
                .outputs
                .into_iter()
                .map(|(name, data)| {
                    (
                        name,
                        PinValue {
                            value: data.val,
                            is_connected: false,
                        },
                    )
                })
                .collect();

            blocks.insert(
                id.to_string(),
                ProgramBlock {
                    name: handle.name.clone(),
                    lib: handle.library.clone(),
                    label: handle.label.clone(),
                    positions: handle.position,
                    inputs,
                    outputs,
                },
            );

            for link in block_links {
                let link_id = link
                    .id
                    .clone()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                links.insert(link_id, link);
            }
        }

        Ok(Program {
            name: None,
            description: None,
            blocks,
            links,
        })
    }

    /// Wires `links` — `program`'s, validated by [`stage_program`] —
    /// then pushes `program`'s saved input/output values to its blocks.
    pub(super) async fn apply_program(
        &self,
        program: &Program,
        links: Vec<LinkData>,
    ) -> Result<()> {
        // Wire each link asynchronously (mailbox round-trips between
        // source and target blocks).
        for link in links {
            self.connect_blocks(&link).await?;
        }

        // Push each block's saved input/output values.
        for (uuid_str, pb) in &program.blocks {
            let id = parse_block_uuid(uuid_str)?;
            for (name, pin) in &pb.inputs {
                if has_initial_value(&pin.value) {
                    let _ = self.write_input(&id, name.clone(), pin.value.clone()).await;
                }
            }
            for (name, pin) in &pb.outputs {
                if has_initial_value(&pin.value) {
                    let _ = self
                        .write_output(&id, name.clone(), pin.value.clone())
                        .await;
                }
            }
        }

        Ok(())
    }

    /// Disconnects the link `link_id`, reporting whether it was found.
    pub(super) async fn disconnect_link_by_id(&self, link_id: &Uuid) -> Result<bool> {
        // Try every block. The link belongs to exactly one (its source).
        for handle in self.handles.values() {
            let (reply, response) = oneshot::channel();
            // A block whose task is gone simply cannot own the link; skip
            // it rather than failing the whole search.
            if handle
                .mailbox
                .send(BlockMailboxCmd::DisconnectLink {
                    link_id: *link_id,
                    reply,
                })
                .await
                .is_err()
            {
                continue;
            }
            let targets = response
                .await
                .map_err(|_| EngineError::BlockDroppedReply { id: *handle.id() })?;
            if !targets.is_empty() {
                for (other_id, input_name) in targets {
                    self.decrement_input(&other_id, input_name).await;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// An engine, in either phase, as the graph's operations that create
/// blocks see it: its [`BlockGraph`], and its way of scheduling a block.
pub(super) trait ProgramHost {
    /// The engine's block graph.
    fn graph(&self) -> &BlockGraph;

    /// The engine's block graph, mutably.
    fn graph_mut(&mut self) -> &mut BlockGraph;

    /// Constructs the block `desc` describes under `id` and schedules
    /// it, as the engine's [`schedule`](crate::base::engine::Engine::schedule)
    /// does.
    fn schedule_desc(&mut self, desc: &BlockDesc, id: Uuid) -> Result<Uuid>;
}

/// Schedules every block of `program` on `host`, recording its UI
/// metadata, and returns the program's links once validated, ready to
/// wire.
///
/// Every block id is checked before anything is scheduled, so a
/// program naming an invalid or already-scheduled id schedules no
/// block at all.
pub(super) fn stage_program<H: ProgramHost>(
    host: &mut H,
    program: &Program,
) -> Result<Vec<LinkData>> {
    let blocks = program_block_ids(program, |id| host.graph().contains(id))?;
    for (id, pb) in blocks {
        let block_def =
            get_block(&pb.name, Some(&pb.lib)).ok_or_else(|| RegistryError::BlockNotFound {
                library: pb.lib.clone(),
                name: pb.name.clone(),
            })?;
        host.schedule_desc(&block_def.desc, id)?;
        // Record UI metadata on the handle so it round-trips on save.
        if let Some(handle) = host.graph_mut().handles.get_mut(&id) {
            handle.label = pb.label.clone();
            handle.position = pb.positions;
        }
    }
    let graph = host.graph();
    program
        .links
        .values()
        .map(|link| graph.validate_link(link))
        .collect()
}

/// Filter for "this is a real saved value, not a placeholder."
/// Programs sometimes annotate a computed output pin with `{}` purely
/// as a UI hint that the pin exists. Loading that verbatim would push
/// an empty Dict into the value, which then propagates through
/// connected links to Number-typed downstream pins and faults their
/// drain.
fn has_initial_value(value: &Value) -> bool {
    !matches!(value, Value::Null)
        && !matches!(value, Value::Dict(d) if d.is_empty())
        && !matches!(value, Value::List(l) if l.is_empty())
}
