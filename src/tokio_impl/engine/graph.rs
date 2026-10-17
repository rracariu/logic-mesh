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
//!
//! ## Links
//!
//! A link lives on its source block, inside the source's actor. The
//! graph also keeps an index of every link by id, with both of its
//! ends: it is how a link id stays unique per engine — a caller may
//! name the id of a link it creates, and a program load keeps the ids
//! the program was saved with — and how a link is found by id without
//! asking every block. The index mirrors the actors: it records the
//! links a block was scheduled with, and every link the graph wires
//! or removes.

use std::collections::{BTreeMap, BTreeSet};

use libhaystack::val::Value;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use super::block_mailbox::{BlockMailboxCmd, mailbox_request, mailbox_send, terminate_actor};
use super::program_block_ids;
use crate::base::block::{BlockDesc, BlockProps};
use crate::base::engine::messages::BlockDefinition;
use crate::base::error::{
    EngineError, LinkEnd, RegistryError, Result, parse_block_uuid, parse_link_uuid,
};
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

/// One link, by its two ends.
#[derive(Debug, Clone)]
pub(super) struct LinkRecord {
    /// The block the link reads from, which holds the link.
    source: Uuid,
    /// The source block's output — or input, for input fan-out.
    source_pin: String,
    /// The block the link writes to.
    target: Uuid,
    /// The target block's input.
    target_pin: String,
}

/// An engine's scheduled blocks, keyed by block id, and the links
/// between them, keyed by link id.
#[derive(Default)]
pub(super) struct BlockGraph {
    handles: BTreeMap<Uuid, BlockHandle>,
    /// Every link, mirroring the links the actors hold — see the
    /// [module docs](self#links).
    links: BTreeMap<Uuid, LinkRecord>,
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
    /// `mailbox`, along with the links it was wired with before it was
    /// scheduled.
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
        for (pin, links) in block.links() {
            for link in links {
                let record = LinkRecord {
                    source: id,
                    source_pin: pin.to_string(),
                    target: *link.target_block_id(),
                    target_pin: link.target_input().to_string(),
                };
                self.links.insert(*link.id(), record);
            }
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

    /// Removes every block and link, telling each actor to terminate.
    ///
    /// The actors are told without waiting on their mailboxes: on a
    /// single-threaded engine no actor makes progress while the engine
    /// resets, so one full mailbox would block an awaited send for good.
    pub(super) fn clear(&mut self) {
        for (_, handle) in std::mem::take(&mut self.handles) {
            terminate_actor(handle.mailbox);
        }
        self.links.clear();
    }

    /// Validates `link_data` against the scheduled blocks and the links
    /// already in place, returning the link's id — the one it names, or
    /// a fresh one if it names none — and its record.
    ///
    /// Wiring requires mailbox round-trips between the source and target
    /// actor tasks, which only answer while the engine runs. This checks
    /// synchronously that the id is free and that both blocks and the
    /// named pins exist, by consulting each block's static `BlockDesc`.
    pub(super) fn validate_link(&self, link_data: &LinkData) -> Result<(Uuid, LinkRecord)> {
        let id = match link_data.id.as_deref() {
            Some(id) => parse_link_uuid(id)?,
            None => Uuid::new_v4(),
        };
        if self.links.contains_key(&id) {
            return Err(EngineError::DuplicateLinkId { id }.into());
        }
        let record = link_record(link_data, |_, block| {
            self.handle_or_err(block).map(BlockHandle::desc)
        })?;
        Ok((id, record))
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

    /// Connects two blocks (source pin → target input), returning the
    /// link with its id. The source pin can be either an output or an
    /// input (the latter is input-fanout).
    ///
    /// The link keeps the id `link_data` names, if any; otherwise it
    /// gets a fresh one.
    ///
    /// # Errors
    ///
    /// Besides the wiring failures, an id that does not parse
    /// ([`EngineError::InvalidLinkId`]) or that another link already
    /// has ([`EngineError::DuplicateLinkId`]) is rejected, as is a
    /// block or pin that does not exist.
    pub(super) async fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData> {
        let (id, record) = self.validate_link(link_data)?;
        self.wire(id, &record).await?;
        self.links.insert(id, record);
        Ok(LinkData {
            id: Some(id.to_string()),
            ..link_data.clone()
        })
    }

    /// Wires link `id` between the actors at its ends: the source block
    /// gets the link, the target input counts it and is seeded with the
    /// source's current value. Leaves the index to the caller.
    ///
    /// A link the source took is taken back if the target cannot be
    /// wired, so a failure leaves no half-wired link behind.
    async fn wire(&self, id: Uuid, link: &LinkRecord) -> Result<()> {
        let source_mb = self.mailbox_or_err(&link.source)?;
        let target_mb = self.mailbox_or_err(&link.target)?;

        // Get the writer of the target input.
        let target_writer = mailbox_request(target_mb, link.target, |reply| {
            BlockMailboxCmd::GetInputWriter {
                name: link.target_pin.clone(),
                reply,
            }
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)?;

        // Try AddOutputLink first; if the source pin is not an output, fall
        // back to AddInputLink.
        let is_output =
            mailbox_request(source_mb, link.source, |reply| BlockMailboxCmd::HasOutput {
                name: link.source_pin.clone(),
                reply,
            })
            .await?;

        mailbox_request(source_mb, link.source, |reply| {
            if is_output {
                BlockMailboxCmd::AddOutputLink {
                    link_id: id,
                    output_name: link.source_pin.clone(),
                    target_block_id: link.target,
                    target_input_name: link.target_pin.clone(),
                    target_writer,
                    reply,
                }
            } else {
                BlockMailboxCmd::AddInputLink {
                    link_id: id,
                    input_name: link.source_pin.clone(),
                    target_block_id: link.target,
                    target_input_name: link.target_pin.clone(),
                    target_writer,
                    reply,
                }
            }
        })
        .await?
        .map_err(EngineError::BlockRequestRejected)?;

        if let Err(err) = self.attach_target(link, is_output).await {
            let _ = mailbox_request(source_mb, link.source, |reply| {
                BlockMailboxCmd::DisconnectLink { link_id: id, reply }
            })
            .await;
            return Err(err);
        }
        Ok(())
    }

    /// The target half of [`wire`](Self::wire), once the source holds
    /// the link.
    async fn attach_target(&self, link: &LinkRecord, is_output: bool) -> Result<()> {
        let target_mb = self.mailbox_or_err(&link.target)?;

        // Increment the target's connection count for this input. Without
        // this, the input would read as unconnected — no input wait wakes
        // on it — and `is_connected` would misreport it. The new count
        // itself is of no interest, so only the delivery half of the
        // round-trip is checked.
        let (inc_reply, inc_response) = oneshot::channel();
        mailbox_send(
            target_mb,
            link.target,
            BlockMailboxCmd::IncrementInput {
                name: link.target_pin.clone(),
                reply: inc_reply,
            },
        )
        .await?;
        let _ = inc_response.await;

        // Seed the target input with the source's current value so the
        // newly-connected block sees a value immediately.
        let seed_value = self
            .read_source_value(&link.source, &link.source_pin, is_output)
            .await
            .unwrap_or_default();
        mailbox_send(
            target_mb,
            link.target,
            BlockMailboxCmd::SeedInputValue {
                name: link.target_pin.clone(),
                value: seed_value,
            },
        )
        .await?;
        // Refresh another connected input on the target so the block
        // re-cycles with the freshly-seeded value.
        self.reset_connected_inputs(&link.target, &link.target_pin)
            .await;
        Ok(())
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
        self.links
            .retain(|_, link| link.source != *block_id && link.target != *block_id);

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

    /// Wires `links` — `program`'s, staged by [`stage_program`] — then
    /// pushes `program`'s saved input/output values to its blocks.
    pub(super) async fn apply_program(
        &mut self,
        program: &Program,
        links: Vec<(Uuid, LinkRecord)>,
    ) -> Result<()> {
        // Wire each link asynchronously (mailbox round-trips between
        // source and target blocks). The staged links already hold their
        // ids in the index: a link that is not wired gives its id back.
        let mut links = links.into_iter();
        while let Some((id, link)) = links.next() {
            if let Err(err) = self.wire(id, &link).await {
                self.links.remove(&id);
                for (id, _) in links {
                    self.links.remove(&id);
                }
                return Err(err);
            }
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

    /// Wires the links staged while the engine was idle, once its actors
    /// answer. A link that cannot be wired is dropped.
    pub(super) async fn wire_staged(&mut self, links: Vec<(Uuid, LinkRecord)>) {
        for (id, link) in links {
            if self.wire(id, &link).await.is_err() {
                self.links.remove(&id);
            }
        }
    }

    /// Disconnects the link `link_id`, reporting whether it was found.
    pub(super) async fn disconnect_link_by_id(&mut self, link_id: &Uuid) -> Result<bool> {
        let Some(link) = self.links.get(link_id) else {
            return Ok(false);
        };
        // The link lives on its source block.
        let source_mb = self.mailbox_or_err(&link.source)?;
        let targets = mailbox_request(source_mb, link.source, |reply| {
            BlockMailboxCmd::DisconnectLink {
                link_id: *link_id,
                reply,
            }
        })
        .await?;
        self.links.remove(link_id);
        let found = !targets.is_empty();
        for (other_id, input_name) in targets {
            self.decrement_input(&other_id, input_name).await;
        }
        Ok(found)
    }
}

/// The record of `link`, once both of its ends check out: a block
/// `desc_of` resolves, and a pin of that block — an output or, for
/// input fan-out, an input at the source; an input at the target.
fn link_record<'a>(
    link: &LinkData,
    desc_of: impl Fn(LinkEnd, &Uuid) -> Result<&'a BlockDesc, EngineError>,
) -> Result<LinkRecord, EngineError> {
    let source = parse_block_uuid(&link.source_block_uuid)?;
    let target = parse_block_uuid(&link.target_block_uuid)?;
    let source_desc = desc_of(LinkEnd::Source, &source)?;
    let target_desc = desc_of(LinkEnd::Target, &target)?;

    let source_pin = link.source_block_pin_name.as_str();
    let source_pin_exists = source_desc.outputs.iter().any(|o| o.name == source_pin)
        || source_desc.inputs.iter().any(|i| i.name == source_pin);
    if !source_pin_exists {
        return Err(EngineError::PinNotFound {
            end: LinkEnd::Source,
            block: source,
            pin: source_pin.to_string(),
        });
    }

    let target_pin = link.target_block_pin_name.as_str();
    if !target_desc.inputs.iter().any(|i| i.name == target_pin) {
        return Err(EngineError::PinNotFound {
            end: LinkEnd::Target,
            block: target,
            pin: target_pin.to_string(),
        });
    }

    Ok(LinkRecord {
        source,
        source_pin: source_pin.to_string(),
        target,
        target_pin: target_pin.to_string(),
    })
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
/// wire. Their ids are taken in the index from here on.
///
/// Every block id is checked before anything is scheduled, so a
/// program naming an invalid or already-scheduled id schedules no
/// block at all.
///
/// A link keeps the id the program saved it under: its `id`, or else
/// the key the program's links are keyed by.
pub(super) fn stage_program<H: ProgramHost>(
    host: &mut H,
    program: &Program,
) -> Result<Vec<(Uuid, LinkRecord)>> {
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
    let graph = host.graph_mut();
    let mut ids = BTreeSet::new();
    let mut links = Vec::new();
    for (key, link) in &program.links {
        let id = parse_link_uuid(link.id.as_deref().unwrap_or(key))?;
        if graph.links.contains_key(&id) || !ids.insert(id) {
            return Err(EngineError::DuplicateLinkId { id }.into());
        }
        let record = link_record(link, |_, block| {
            graph.handle_or_err(block).map(BlockHandle::desc)
        })?;
        links.push((id, record));
    }
    for (id, link) in &links {
        graph.links.insert(*id, link.clone());
    }
    Ok(links)
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
