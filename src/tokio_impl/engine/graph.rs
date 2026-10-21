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
use std::fmt;

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

/// Shows the block's identity and UI metadata, not its descriptor or
/// mailbox.
impl fmt::Debug for BlockHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockHandle")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("library", &self.library)
            .field("label", &self.label)
            .field("position", &self.position)
            .finish_non_exhaustive()
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

impl LinkRecord {
    /// The link, as a program saves it under `id`.
    fn to_link_data(&self, id: &Uuid) -> LinkData {
        LinkData {
            id: Some(id.to_string()),
            source_block_uuid: self.source.to_string(),
            target_block_uuid: self.target.to_string(),
            source_block_pin_name: self.source_pin.clone(),
            target_block_pin_name: self.target_pin.clone(),
        }
    }
}

/// A block's saved inputs and outputs, by pin name, and its outgoing
/// links.
type SavedPins = (
    BTreeMap<String, PinValue>,
    BTreeMap<String, PinValue>,
    Vec<LinkData>,
);

/// An engine's scheduled blocks, keyed by block id, and the links
/// between them, keyed by link id.
#[derive(Default)]
pub(super) struct BlockGraph {
    handles: BTreeMap<Uuid, BlockHandle>,
    /// Every link, mirroring the links the actors hold — see the
    /// [module docs](self#links).
    links: BTreeMap<Uuid, LinkRecord>,
    /// The [`Program::name`] of the program loaded last, until a reset.
    /// Pure passthrough, like a block's label: kept so a save returns it.
    name: Option<String>,
    /// The [`Program::description`] of the program loaded last, until a
    /// reset. Same passthrough role as `name`.
    description: Option<String>,
}

impl BlockGraph {
    /// The number of scheduled blocks.
    pub(super) fn block_count(&self) -> usize {
        self.handles.len()
    }

    /// The number of links between scheduled blocks.
    pub(super) fn link_count(&self) -> usize {
        self.links.len()
    }

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

    /// Records `name` and `description` as those of the program the
    /// engine runs, replacing any earlier program's.
    pub(super) fn set_identity(&mut self, name: Option<String>, description: Option<String>) {
        self.name = name;
        self.description = description;
    }

    /// Removes every block and link, and the loaded program's identity,
    /// telling each actor to terminate.
    ///
    /// The actors are told without waiting on their mailboxes: on a
    /// single-threaded engine no actor makes progress while the engine
    /// resets, so one full mailbox would block an awaited send for good.
    pub(super) fn clear(&mut self) {
        for (_, handle) in std::mem::take(&mut self.handles) {
            terminate_actor(handle.mailbox);
        }
        self.links.clear();
        self.name = None;
        self.description = None;
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
    /// ([`EngineError::InvalidLinkUuid`]) or that another link already
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
        .await??;

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
        .map_err(EngineError::Connect)?;

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

    /// Removes block `block_id` and all its links — even when the
    /// block's actor is gone already, for whatever reason: its handle and
    /// its links are dropped all the same, and the inputs it fed are
    /// updated.
    pub(super) async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        // 1. Tell the target to disconnect itself; collect targets that need
        //    decrementing. A block whose actor is gone took its links with
        //    it, but the index still knows where they went.
        let target_mb = self.mailbox_or_err(block_id)?;
        let targets = match mailbox_request(target_mb, *block_id, |reply| {
            BlockMailboxCmd::DisconnectAll { reply }
        })
        .await
        {
            Ok(targets) => targets,
            Err(err) => {
                log::warn!("Removing block {block_id}, whose actor is gone: {err}");
                self.links
                    .values()
                    .filter(|link| link.source == *block_id)
                    .map(|link| (link.target, link.target_pin.clone()))
                    .collect()
            }
        };

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

        // 3. Terminate the block, unless its actor is gone already.
        if let Some(handle) = self.handles.remove(block_id) {
            terminate_actor(handle.mailbox);
        }
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

    /// Snapshot the full program: the loaded program's name and
    /// description, every scheduled block with its UI metadata + current
    /// pin values, plus every link under its id. This is the canonical
    /// save format — round-trips through a program load.
    ///
    /// A block whose actor is gone is saved from what its handle and
    /// the link index still know — its type, label, position and
    /// outgoing links — with no pin values: those were lost with the
    /// actor. Failing the whole save instead would lose the rest of the
    /// program as well, and removing the block, the only way out,
    /// would lose its links too. The loss is logged.
    pub(super) async fn save_program(&self) -> Result<Program> {
        let mut blocks = BTreeMap::new();
        let mut links = BTreeMap::new();

        for (id, handle) in &self.handles {
            let (inputs, outputs, block_links) = match self.snapshot(handle).await {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    log::warn!(
                        "Saving block {id} ({}) without its pin values: {err}",
                        handle.name
                    );
                    let block_links = self
                        .links
                        .iter()
                        .filter(|(_, link)| link.source == *id)
                        .map(|(link_id, link)| link.to_link_data(link_id))
                        .collect();
                    (BTreeMap::new(), BTreeMap::new(), block_links)
                }
            };

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
            name: self.name.clone(),
            description: self.description.clone(),
            blocks,
            links,
        })
    }

    /// The pin values and outgoing links of the block behind `handle`,
    /// read from its actor.
    async fn snapshot(&self, handle: &BlockHandle) -> Result<SavedPins, EngineError> {
        let id = handle.id;
        // Pull dynamic state (pin values + per-input connectedness)
        // via `Inspect`.
        let definition = mailbox_request(&handle.mailbox, id, |reply| BlockMailboxCmd::Inspect {
            reply,
        })
        .await?;

        // Pull outgoing links via `GetBlockData`.
        let (_block_data, block_links) = mailbox_request(&handle.mailbox, id, |reply| {
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
        Ok((inputs, outputs, block_links))
    }

    /// Completes the load of a program [`stage_program`] staged: wires
    /// its links, then pushes its saved pin values.
    ///
    /// All or nothing: validation caught every failure it could before
    /// the blocks were scheduled, so what can still fail here is the
    /// wiring itself — a link the actors reject, an actor that is gone.
    /// Then the program's blocks are removed again, links and all, and
    /// the engine is left as it was before the load. The blocks may
    /// have run in between, so an effect one of them had outside the
    /// engine — a connector publish, say — is not undone.
    pub(super) async fn load_staged(&mut self, staged: StagedProgram) -> Result<()> {
        if let Err(err) = self.apply_staged(&staged).await {
            self.discard(&staged.blocks);
            return Err(err);
        }
        self.set_identity(staged.name, staged.description);
        Ok(())
    }

    /// The fallible half of [`load_staged`](Self::load_staged).
    async fn apply_staged(&self, staged: &StagedProgram) -> Result<()> {
        for (id, link) in &staged.links {
            self.wire(*id, link).await?;
        }
        for pin in &staged.pins {
            self.write_pin(pin).await?;
        }
        Ok(())
    }

    /// Wires the links and pushes the pin values of a program staged
    /// while the engine was idle, once its actors answer.
    ///
    /// Its blocks are scheduled already, so there is nothing to roll
    /// back to: validation caught every failure it could up front, and
    /// a link that still cannot be wired — or a pin that cannot be
    /// written — is dropped with a warning.
    pub(super) async fn complete_staged(&mut self, staged: StagedProgram) {
        for (id, link) in staged.links {
            if let Err(err) = self.wire(id, &link).await {
                log::warn!("Dropping link {id} of a scheduled program: {err}");
                self.links.remove(&id);
            }
        }
        for pin in &staged.pins {
            if let Err(err) = self.write_pin(pin).await {
                log::warn!(
                    "Dropping the value of {}.{} of a scheduled program: {err}",
                    pin.block,
                    pin.name,
                );
            }
        }
    }

    /// Pushes one saved pin value.
    async fn write_pin(&self, pin: &PinWrite) -> Result<(), EngineError> {
        let name = pin.name.clone();
        let value = pin.value.clone();
        if pin.output {
            self.write_output(&pin.block, name, value).await?;
        } else {
            self.write_input(&pin.block, name, value).await?;
        }
        Ok(())
    }

    /// Removes `blocks`, freshly scheduled by a load that failed, and
    /// every link touching them.
    fn discard(&mut self, blocks: &[Uuid]) {
        for id in blocks {
            if let Some(handle) = self.handles.remove(id) {
                terminate_actor(handle.mailbox);
            }
        }
        self.links
            .retain(|_, link| !blocks.contains(&link.source) && !blocks.contains(&link.target));
    }

    /// Disconnects the link `link_id`, reporting whether it was found.
    ///
    /// The link lives on its source block. A source whose actor is gone
    /// took the link with it, so only its target is left to update.
    pub(super) async fn disconnect_link_by_id(&mut self, link_id: &Uuid) -> Result<bool> {
        let Some(link) = self.links.remove(link_id) else {
            return Ok(false);
        };
        let gone = || vec![(link.target, link.target_pin.clone())];
        let targets = match self.mailbox(&link.source) {
            Some(source_mb) => match mailbox_request(source_mb, link.source, |reply| {
                BlockMailboxCmd::DisconnectLink {
                    link_id: *link_id,
                    reply,
                }
            })
            .await
            {
                Ok(targets) => targets,
                Err(err) => {
                    log::warn!("Removing link {link_id}, whose source actor is gone: {err}");
                    gone()
                }
            },
            None => gone(),
        };
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

    /// Checks, without constructing anything, that
    /// [`schedule_desc`](Self::schedule_desc) supports the kind of block
    /// `desc` describes.
    fn ensure_schedulable(&self, desc: &BlockDesc) -> Result<()>;

    /// Constructs the block `desc` describes under `id` and schedules
    /// it, as the engine's [`schedule`](crate::base::engine::Engine::schedule)
    /// does.
    fn schedule_desc(&mut self, desc: &BlockDesc, id: Uuid) -> Result<Uuid>;
}

/// One saved pin value of a program, to push once its block is wired.
#[derive(Debug)]
struct PinWrite {
    /// The block the pin belongs to.
    block: Uuid,
    /// The pin name.
    name: String,
    /// The saved value.
    value: Value,
    /// Whether the pin is an output rather than an input.
    output: bool,
}

/// A program whose blocks [`stage_program`] scheduled: what is left to
/// do once the actors answer, and the program's identity, to record
/// once that is done.
pub(super) struct StagedProgram {
    /// The program's blocks, as scheduled.
    blocks: Vec<Uuid>,
    /// The program's links, their ids taken in the index already.
    links: Vec<(Uuid, LinkRecord)>,
    /// The program's saved pin values.
    pins: Vec<PinWrite>,
    /// [`Program::name`].
    name: Option<String>,
    /// [`Program::description`].
    description: Option<String>,
}

impl StagedProgram {
    /// The program's name and description.
    pub(super) fn identity(&self) -> (Option<String>, Option<String>) {
        (self.name.clone(), self.description.clone())
    }
}

/// A program that passed every check of [`stage_program`], with no
/// block scheduled yet.
struct CheckedProgram<'p> {
    /// Every block, with its descriptor.
    blocks: Vec<(Uuid, &'p ProgramBlock, BlockDesc)>,
    /// Every link, by id.
    links: Vec<(Uuid, LinkRecord)>,
    /// Every saved pin value worth pushing.
    pins: Vec<PinWrite>,
}

/// Checks all of `program` against the registry and `host`'s graph,
/// then schedules its blocks, recording their UI metadata, and takes
/// its link ids — or changes nothing and fails.
///
/// The checks cover everything that can be checked before a block
/// exists: every block id parses and is free, on the engine and in the
/// program; every block type resolves, and `host` can schedule it;
/// every pin a saved value names exists on its block
/// ([`EngineError::InputNotFound`], [`EngineError::OutputNotFound`]);
/// and every link has an id that parses and is free, joins two pins
/// that exist on blocks of the same program
/// ([`EngineError::LinkBlockNotInProgram`]), and joins pins no other of
/// its links joins ([`EngineError::DuplicateLink`]). Should scheduling
/// a block still fail, the blocks scheduled before it are removed
/// again.
///
/// A link keeps the id the program saved it under: its `id`, or else
/// the key the program's links are keyed by.
pub(super) fn stage_program<H: ProgramHost>(
    host: &mut H,
    program: &Program,
) -> Result<StagedProgram> {
    let checked = check_program(host, program)?;

    let mut scheduled = Vec::with_capacity(checked.blocks.len());
    for (id, block, desc) in &checked.blocks {
        if let Err(err) = host.schedule_desc(desc, *id) {
            host.graph_mut().discard(&scheduled);
            return Err(err);
        }
        scheduled.push(*id);
        // Record UI metadata on the handle so it round-trips on save.
        if let Some(handle) = host.graph_mut().handles.get_mut(id) {
            handle.label = block.label.clone();
            handle.position = block.positions;
        }
    }

    let graph = host.graph_mut();
    for (id, link) in &checked.links {
        graph.links.insert(*id, link.clone());
    }
    Ok(StagedProgram {
        blocks: scheduled,
        links: checked.links,
        pins: checked.pins,
        name: program.name.clone(),
        description: program.description.clone(),
    })
}

/// The checks of [`stage_program`].
fn check_program<'p, H: ProgramHost>(host: &H, program: &'p Program) -> Result<CheckedProgram<'p>> {
    let graph = host.graph();
    let mut blocks = Vec::new();
    let mut pins = Vec::new();
    for (id, block) in program_block_ids(program, |id| graph.contains(id))? {
        let entry = get_block(&block.name, Some(&block.lib)).ok_or_else(|| {
            RegistryError::BlockNotFound {
                library: block.lib.clone(),
                name: block.name.clone(),
            }
        })?;
        host.ensure_schedulable(&entry.desc)?;
        for (output, pins_of, values) in [
            (false, &entry.desc.inputs, &block.inputs),
            (true, &entry.desc.outputs, &block.outputs),
        ] {
            for (name, value) in values {
                if !pins_of.iter().any(|pin| pin.name == *name) {
                    let (block, pin) = (id, name.clone());
                    return Err(if output {
                        EngineError::OutputNotFound { block, pin }
                    } else {
                        EngineError::InputNotFound { block, pin }
                    }
                    .into());
                }
                if has_initial_value(&value.value) {
                    pins.push(PinWrite {
                        block: id,
                        name: name.clone(),
                        value: value.value.clone(),
                        output,
                    });
                }
            }
        }
        blocks.push((id, block, entry.desc));
    }

    let descs: BTreeMap<_, _> = blocks.iter().map(|(id, _, desc)| (*id, desc)).collect();
    let mut ids = BTreeSet::new();
    let mut joined = BTreeMap::new();
    let mut links = Vec::new();
    for (key, link) in &program.links {
        let id = parse_link_uuid(link.id.as_deref().unwrap_or(key))?;
        if graph.links.contains_key(&id) || !ids.insert(id) {
            return Err(EngineError::DuplicateLinkId { id }.into());
        }
        let record = link_record(link, |end, block| {
            descs
                .get(block)
                .copied()
                .ok_or(EngineError::LinkBlockNotInProgram {
                    link: id,
                    end,
                    block: *block,
                })
        })?;
        let ends = (
            record.source,
            record.source_pin.clone(),
            record.target,
            record.target_pin.clone(),
        );
        if let Some(existing) = joined.insert(ends, id) {
            return Err(EngineError::DuplicateLink { link: id, existing }.into());
        }
        links.push((id, record));
    }

    Ok(CheckedProgram {
        blocks,
        links,
        pins,
    })
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
