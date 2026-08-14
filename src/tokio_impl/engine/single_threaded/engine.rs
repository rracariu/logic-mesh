// Copyright (c) 2022-2026, Radu Racariu.

//! [`SingleThreadedEngine`] — engine struct, lifecycle, and the
//! synchronous + async API surface that messaging dispatch routes to.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use crate::base::error::{EngineError, LinkEnd, RegistryError, Result, parse_block_uuid};
use libhaystack::val::Value;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender, UnboundedSender},
        oneshot,
    },
    task::LocalSet,
};
use uuid::Uuid;

use super::super::block_mailbox::{
    BLOCK_MAILBOX_CAP, BlockMailboxCmd, mailbox_request, mailbox_send,
};
use super::actor::block_actor_task;
use crate::base::{
    block::{Block, BlockDesc},
    engine::{
        Engine,
        messages::{BlockDefinition, EngineMessage, WatchMessage},
    },
    program::{
        Program,
        data::{LinkData, PinValue, Position, ProgramBlock},
    },
};
use crate::blocks::registry::{CORE_LIB, get_block};
use crate::tokio_impl::engine::message_dispatch::dispatch_message;
use crate::tokio_impl::engine::schedule_block_on_engine;
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Concrete engine-message type.
///
/// The watch-event sender is unbounded — see `wasm/engine_command.rs`
/// `create_watch` for the rationale (fault notifications must not drop
/// under burst load).
pub type Messages = EngineMessage<UnboundedSender<WatchMessage>>;

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
    /// We clone instead of borrowing `&'static BlockDesc` because for
    /// JS blocks the descriptor lives inside the block instance — and
    /// the instance gets moved into its actor task at schedule time,
    /// invalidating any borrowed pointer. Native (Rust) blocks back
    /// their descriptor with a `LazyLock<BlockDesc>` so a `&'static`
    /// would be sound for them, but we use a single uniform owned shape
    /// to avoid an unsafe trait split. The clone happens once per
    /// block at schedule time.
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

/// Single-threaded execution environment for blocks.
pub struct SingleThreadedEngine {
    /// LocalSet that hosts every per-block actor task. Wrapped in `Rc` so
    /// `run()` can clone a handle and avoid double-borrowing `self` when it
    /// needs to drive the local set while also dispatching engine messages.
    local: Rc<LocalSet>,
    /// Active block handles, keyed by block id. Populated by `schedule()`
    /// (each block spawns its actor task immediately).
    handles: BTreeMap<Uuid, BlockHandle>,
    /// Links queued during configuration; processed at `run()` start (which
    /// pumps the LocalSet so the actor tasks can answer the round-trips).
    pending_links: Vec<LinkData>,
    /// External-command channel (receiving end).
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
    /// Reply channels for external command issuers. `pub(in super::super)`
    /// so the message-dispatch loop (sibling of `single_threaded/`) can
    /// iterate it to deliver replies.
    pub(in super::super) reply_senders: BTreeMap<Uuid, Sender<Messages>>,
    /// Watchers for change-of-value notifications. Same visibility note as
    /// `reply_senders`.
    pub(in super::super) watchers: Rc<RefCell<BTreeMap<Uuid, UnboundedSender<WatchMessage>>>>,
}

impl Default for SingleThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for SingleThreadedEngine {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type Channel = Sender<Messages>;

    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        block: B,
    ) -> Result<()> {
        let id = *block.id();
        let name = block.name().to_string();
        let library = block.desc().library.clone();
        // Clone the desc into the handle BEFORE moving `block` into the
        // actor task. JS blocks' "static" desc actually lives inside the
        // block instance; borrowing it across the move is UB.
        let desc: BlockDesc = block.desc().clone();
        let (mailbox_tx, mailbox_rx) = mpsc::channel::<BlockMailboxCmd>(BLOCK_MAILBOX_CAP);

        let handle = BlockHandle {
            id,
            name,
            library,
            desc,
            mailbox: mailbox_tx,
            label: None,
            position: None,
        };
        self.handles.insert(id, handle);

        let watchers = self.watchers.clone();
        self.local
            .spawn_local(block_actor_task(block, mailbox_rx, watchers));
        Ok(())
    }

    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()> {
        for (uuid_str, pb) in &program.blocks {
            let id = parse_block_uuid(uuid_str)?;
            let block_def =
                get_block(&pb.name, Some(&pb.lib)).ok_or_else(|| RegistryError::BlockNotFound {
                    library: pb.lib.clone(),
                    name: pb.name.clone(),
                })?;
            schedule_block_on_engine(&block_def.desc, Some(id), self)?;
            // Record UI metadata on the handle so it round-trips on save.
            if let Some(handle) = self.handles.get_mut(&id) {
                handle.label = pb.label.clone();
                handle.position = pb.positions;
            }
        }
        // Wiring is async (mailbox round-trips); validate sync and queue
        // for processing at `run()` start.
        for link in program.links.values() {
            self.connect_blocks_sync(link)?;
        }
        Ok(())
    }

    async fn run(&mut self) {
        // Clone the LocalSet handle so the loop body can drive `local`
        // and mutate `self` independently of each other.
        let local = self.local.clone();

        // Process any links queued during configuration.
        let pending_links = std::mem::take(&mut self.pending_links);
        for link in pending_links {
            let _ = local.run_until(self.connect_blocks(&link)).await;
        }

        let mut is_paused = false;
        loop {
            let mut engine_msg = None;
            if !is_paused {
                local
                    .run_until(async {
                        engine_msg = self.receiver.recv().await;
                    })
                    .await;
            } else {
                engine_msg = self.receiver.recv().await;
            }

            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    break;
                } else if matches!(message, EngineMessage::Reset) {
                    let ids: Vec<Uuid> = self.handles.keys().copied().collect();
                    for id in ids {
                        if let Some(handle) = self.handles.remove(&id) {
                            let _ = handle.mailbox.send(BlockMailboxCmd::Terminate).await;
                        }
                    }
                    continue;
                } else if matches!(message, EngineMessage::Pause) {
                    is_paused = true;
                    continue;
                } else if matches!(message, EngineMessage::Resume) {
                    is_paused = false;
                    continue;
                }

                local.run_until(dispatch_message(self, message)).await;
            }
        }
    }

    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel {
        self.reply_senders.insert(sender_id, sender_channel);
        self.sender.clone()
    }
}

impl SingleThreadedEngine {
    /// Creates a new single-threaded engine.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        Self {
            local: Rc::new(LocalSet::new()),
            handles: BTreeMap::new(),
            pending_links: Vec::new(),
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Rc::default(),
        }
    }

    /// Returns sync metadata handles for every scheduled block. Use the
    /// async snapshot APIs (`inspect_block`, etc.) to read dynamic state.
    pub fn block_handles(&self) -> Vec<&BlockHandle> {
        self.handles.values().collect()
    }

    /// Returns the handle for a specific block, if scheduled.
    pub fn block_handle(&self, id: &Uuid) -> Option<&BlockHandle> {
        self.handles.get(id)
    }

    fn mailbox(&self, id: &Uuid) -> Option<&mpsc::Sender<BlockMailboxCmd>> {
        self.handles.get(id).map(|h| &h.mailbox)
    }

    /// [`block_handle`](Self::block_handle) for callers that treat an
    /// unscheduled id as an error rather than an absence.
    fn block_handle_or_err(&self, id: &Uuid) -> Result<&BlockHandle, EngineError> {
        self.block_handle(id)
            .ok_or(EngineError::BlockInstanceNotFound { id: *id })
    }

    /// [`mailbox`](Self::mailbox), same treatment.
    fn mailbox_or_err(&self, id: &Uuid) -> Result<&mpsc::Sender<BlockMailboxCmd>, EngineError> {
        self.mailbox(id)
            .ok_or(EngineError::BlockInstanceNotFound { id: *id })
    }

    // --- Sync engine ops (configuration phase) -----------------------------

    pub(super) fn connect_blocks_sync(&mut self, link_data: &LinkData) -> Result<LinkData> {
        // Wiring requires mailbox round-trips between the source and target
        // actor tasks; that's deferred to `run()` start. Here we validate
        // synchronously that both blocks and the named pins exist by
        // consulting each block's static `BlockDesc`, then queue the link.
        let source_id = parse_block_uuid(&link_data.source_block_uuid)?;
        let target_id = parse_block_uuid(&link_data.target_block_uuid)?;
        let source_handle = self.block_handle_or_err(&source_id)?;
        let target_handle = self.block_handle_or_err(&target_id)?;

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
        self.pending_links.push(LinkData {
            id: Some(id.clone()),
            ..link_data.clone()
        });
        Ok(LinkData {
            id: Some(id),
            ..link_data.clone()
        })
    }

    // --- Async engine ops (running phase) ----------------------------------

    pub(crate) async fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<&str>,
    ) -> Result<Uuid> {
        let block_def =
            get_block(block_name.as_str(), lib).ok_or_else(|| RegistryError::BlockNotFound {
                library: lib.unwrap_or(CORE_LIB).to_string(),
                name: block_name.clone(),
            })?;
        // schedule_block_on_engine spawns the actor task immediately (via
        // `schedule()`); it returns the assigned block id.
        schedule_block_on_engine(&block_def.desc, block_id, self)
    }

    pub(crate) async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, EngineError> {
        let mailbox = self.mailbox_or_err(id)?;

        mailbox_request(mailbox, *id, |reply| BlockMailboxCmd::Inspect { reply }).await
    }

    pub(crate) async fn write_input(
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

    pub(crate) async fn write_output(
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

    /// Connect two blocks (source pin → target input). The source pin can
    /// be either an output or an input (the latter is input-fanout).
    pub(crate) async fn connect_blocks(&self, link_data: &LinkData) -> Result<LinkData> {
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
        // this, `drain_ready_inputs` would skip the input and values pushed
        // by the source would sit in the watch channel, never reaching
        // `self.val`. The new count itself is of no interest, so only the
        // delivery half of the round-trip is checked.
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
            .await?;

        Ok(LinkData {
            id: Some(link_id.to_string()),
            ..link_data.clone()
        })
    }

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

    /// Re-arm any other connected inputs on the target block by re-sending
    /// their cached values, so the block re-cycles after a new link is
    /// added.
    ///
    /// Skips unconnected inputs — refreshing them is a no-op
    /// (`drain_ready_inputs` filters by `is_connected`) and the mailbox
    /// round-trip would be wasted. `is_connected` rides on the
    /// `Inspect` snapshot since Q4/A3.
    async fn reset_connected_inputs(&self, target_id: &Uuid, ignore_input: &str) -> Result<()> {
        let inputs = match self.inspect_block(target_id).await {
            Ok(def) => def.inputs,
            Err(_) => return Ok(()),
        };
        if let Some((name, _data)) = inputs
            .iter()
            .find(|(name, data)| name.as_str() != ignore_input && data.is_connected)
        {
            let mb = self.mailbox(target_id);
            if let Some(mb) = mb {
                let _ = mb
                    .send(BlockMailboxCmd::RefreshInput { name: name.clone() })
                    .await;
            }
        }
        Ok(())
    }

    pub(crate) async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        // 1. Tell the target to disconnect itself; collect targets that need
        //    decrementing.
        let target_mb = self.mailbox_or_err(block_id)?.clone();
        let targets = mailbox_request(&target_mb, *block_id, |reply| {
            BlockMailboxCmd::DisconnectAll { reply }
        })
        .await?;

        for (other_id, input_name) in targets {
            if let Some(mb) = self.mailbox(&other_id) {
                let (dec_tx, dec_rx) = oneshot::channel();
                let _ = mb
                    .send(BlockMailboxCmd::DecrementInput {
                        name: input_name,
                        reply: dec_tx,
                    })
                    .await;
                let _ = dec_rx.await;
            }
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

    /// Snapshot the full program: every scheduled block with its UI
    /// metadata + current pin values, plus every link. This is the
    /// canonical save format — round-trips through `load_program`.
    pub(crate) async fn save_program(&self) -> Result<Program> {
        let mut blocks = std::collections::BTreeMap::new();
        let mut links = std::collections::BTreeMap::new();

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

    /// Atomically load a full [`Program`]: schedule every block, wire
    /// every link, push initial input/output values, and store UI
    /// metadata. Idempotent re-entry (engine should be empty or the
    /// caller should reset it first via `Reset`).
    ///
    /// Must be called from within the engine `run()` context (the actor
    /// tasks need to be live to handle the WriteInput/Output mailbox
    /// cmds). When invoked through the engine message channel
    /// (`LoadProgramReq`), this is automatic.
    pub(crate) async fn load_program(&mut self, program: Program) -> Result<()> {
        // Sync: schedule blocks + queue links. After this the per-block
        // actor tasks have been spawned and the link wiring is queued
        // for processing by `connect_blocks` calls below.
        self.schedule_program_blocks(&program)?;

        // Wire each queued link asynchronously (mailbox round-trips
        // between source and target blocks).
        let pending_links = std::mem::take(&mut self.pending_links);
        for link in pending_links {
            self.connect_blocks(&link).await?;
        }

        // Push each block's saved input/output values.
        for (uuid_str, pb) in &program.blocks {
            let id = parse_block_uuid(uuid_str)?;
            for (name, pin) in &pb.inputs {
                if hasinitialvalue(&pin.value) {
                    let _ = self.write_input(&id, name.clone(), pin.value.clone()).await;
                }
            }
            for (name, pin) in &pb.outputs {
                if hasinitialvalue(&pin.value) {
                    let _ = self
                        .write_output(&id, name.clone(), pin.value.clone())
                        .await;
                }
            }
        }

        Ok(())
    }

    /// Mutator for UI-metadata sidecar fields. Used by the message
    /// dispatch path when an external caller wants to update a label /
    /// position without going through a full program reload.
    pub fn set_block_metadata(
        &mut self,
        id: &Uuid,
        label: Option<String>,
        position: Option<Position>,
    ) {
        if let Some(handle) = self.handles.get_mut(id) {
            if let Some(l) = label {
                handle.label = Some(l);
            }
            if let Some(p) = position {
                handle.position = Some(p);
            }
        }
    }

    pub(crate) async fn disconnect_link_by_id(&self, link_id: &Uuid) -> Result<bool> {
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
                    if let Some(mb) = self.mailbox(&other_id) {
                        let (dec_tx, dec_rx) = oneshot::channel();
                        let _ = mb
                            .send(BlockMailboxCmd::DecrementInput {
                                name: input_name,
                                reply: dec_tx,
                            })
                            .await;
                        let _ = dec_rx.await;
                    }
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Filter for "this is a real saved value, not a placeholder."
/// Examples sometimes annotate a computed output pin with `{}` purely
/// as a UI hint that the pin exists. Loading that verbatim would push
/// an empty Dict into the wasm value, which then propagates through
/// connected links to Number-typed downstream pins and faults their
/// drain.
///
/// Mirrors the JS-side `hasInitialValue` filter in `Program.ts`.
fn hasinitialvalue(value: &Value) -> bool {
    use libhaystack::val::Value::*;
    !matches!(value, Null)
        && !matches!(value, Dict(d) if d.is_empty())
        && !matches!(value, List(l) if l.is_empty())
}
