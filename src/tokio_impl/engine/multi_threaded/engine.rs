// Copyright (c) 2022-2026, Radu Racariu.

//! Multi-threaded engine.
//!
//! Block actor tasks are spawned directly onto the caller's tokio
//! multi-threaded runtime via [`tokio::spawn`]. The runtime's
//! work-stealing scheduler decides which worker thread runs each task
//! and can migrate them between threads — block actor futures are
//! [`Send`] by construction (trait-method returns carry `+ Send`; see
//! [`crate::base::block::BlockProps`]).
//!
//! Shares the per-block mailbox protocol ([`super::super::block_mailbox`])
//! with the single-threaded engine, so the actor task implementation is
//! near-identical — the only MT-specific bits are watchers
//! (`Arc<RwLock<...>>`) and the cross-thread mailbox semantics, both of
//! which already work over `tokio::spawn`.
//!
//! ## Runtime requirement
//!
//! The engine assumes it is running inside a tokio multi-thread runtime
//! (`#[tokio::main(flavor = "multi_thread")]` or a manually-constructed
//! `Runtime::new`). It does not create worker threads or runtimes of its
//! own — that's the caller's responsibility.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::base::error::{EngineError, LinkEnd, RegistryError, Result, parse_block_uuid};
use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, Receiver, Sender, UnboundedSender},
    oneshot,
};
use uuid::Uuid;

use super::super::block_mailbox::{
    BLOCK_MAILBOX_CAP, BlockMailboxCmd, mailbox_request, mailbox_send,
};
use super::actor::{WatchersHandle, block_actor_task};
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
use crate::tokio_impl::engine::schedule_block_on_engine_mt;
use crate::tokio_impl::{MtBlock, ReaderImpl, WriterImpl};

/// Concrete engine-message type.
///
/// The watch-event sender is unbounded — see `wasm/engine_command.rs`
/// `create_watch` for the rationale.
pub type Messages = EngineMessage<UnboundedSender<WatchMessage>>;

/// Engine-side handle for a scheduled block in the MT engine.
///
/// Same shape as the ST handle. Notably it no longer carries a
/// `worker_idx` — the tokio MT runtime is free to migrate the actor
/// task between worker threads via work-stealing, so a fixed pinning
/// would be misleading.
pub struct BlockHandle {
    id: Uuid,
    name: String,
    library: String,
    /// Owned clone of the block's descriptor — see the ST
    /// `BlockHandle::desc` doc for the rationale (JS blocks).
    desc: BlockDesc,
    mailbox: mpsc::Sender<BlockMailboxCmd>,
    /// See ST `BlockHandle::label`.
    label: Option<String>,
    /// See ST `BlockHandle::position`.
    position: Option<Position>,
}

impl BlockHandle {
    pub fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn library(&self) -> &str {
        &self.library
    }
    pub fn desc(&self) -> &BlockDesc {
        &self.desc
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn position(&self) -> Option<Position> {
        self.position
    }
}

/// Multi-threaded execution environment for blocks.
pub struct MultiThreadedEngine {
    handles: BTreeMap<Uuid, BlockHandle>,
    pending_links: Vec<LinkData>,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
    pub(in super::super) reply_senders: BTreeMap<Uuid, Sender<Messages>>,
    pub(in super::super) watchers: WatchersHandle,
}

impl Default for MultiThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::base::engine::Engine for MultiThreadedEngine {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type Channel = Sender<Messages>;

    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        _block: B,
    ) -> Result<()> {
        // The MT engine requires `Send` because the block crosses the
        // worker-thread boundary, but this trait signature cannot express
        // that bound. Use [`MultiThreadedEngine::schedule_send`] for the
        // Send-bounded entry point.
        Err(EngineError::ScheduleRequiresSend.into())
    }

    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()> {
        for (uuid_str, pb) in &program.blocks {
            let id = parse_block_uuid(uuid_str)?;
            let block_def = get_block(&pb.name, Some(pb.lib.as_str())).ok_or_else(|| {
                RegistryError::BlockNotFound {
                    library: pb.lib.clone(),
                    name: pb.name.clone(),
                }
            })?;
            schedule_block_on_engine_mt(&block_def.desc, Some(id), self)?;
            if let Some(handle) = self.handles.get_mut(&id) {
                handle.label = pb.label.clone();
                handle.position = pb.positions;
            }
        }
        for link in program.links.values() {
            self.connect_blocks_sync(link)?;
        }
        Ok(())
    }

    async fn run(&mut self) {
        // Process any links queued during configuration. Actor tasks are
        // already live on the tokio MT runtime by this point (they were
        // spawned at `schedule_send` time), so the mailbox round-trips
        // resolve immediately.
        let pending_links = std::mem::take(&mut self.pending_links);
        for link in pending_links {
            let _ = self.connect_blocks(&link).await;
        }

        let mut is_paused = false;
        loop {
            let engine_msg = self.receiver.recv().await;
            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    // Tell every actor task to terminate. Dropping the
                    // mailbox senders would also cause the tasks to
                    // notice via `mailbox.recv() -> None`, but explicit
                    // termination is faster and lets the actor record
                    // `BlockState::Terminated` for any in-flight watcher.
                    let ids: Vec<Uuid> = self.handles.keys().copied().collect();
                    for id in ids {
                        if let Some(handle) = self.handles.remove(&id) {
                            let _ = handle.mailbox.send(BlockMailboxCmd::Terminate).await;
                        }
                    }
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

                if !is_paused {
                    self.dispatch_message(message).await;
                }
            }
        }
    }

    fn create_message_channel(
        &mut self,
        sender_id: Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel {
        self.reply_senders.insert(sender_id, sender_channel);
        self.sender.clone()
    }
}

impl MultiThreadedEngine {
    /// Create a new multi-threaded engine. The engine itself does not
    /// own worker threads — actor tasks are spawned onto whichever
    /// tokio multi-thread runtime the engine is running inside of.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);

        Self {
            handles: BTreeMap::new(),
            pending_links: Vec::new(),
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Schedule a block on the engine. The block must be `Send + 'static`
    /// because the actor task is handed to [`tokio::spawn`], where the
    /// runtime is free to migrate it between worker threads.
    ///
    /// Must be called from within a tokio multi-thread runtime context
    /// (`#[tokio::main(flavor = "multi_thread")]`, `Runtime::block_on`,
    /// or an already-spawned task on such a runtime). Calling this
    /// outside a runtime panics — that's a `tokio::spawn` invariant.
    pub fn schedule_send<B>(&mut self, block: B)
    where
        B: MtBlock + 'static,
    {
        let id = *block.id();
        let name = block.name().to_string();
        let library = block.desc().library.clone();
        // See the ST engine's `schedule` for why we clone here.
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
        tokio::spawn(block_actor_task(block, mailbox_rx, watchers));
    }

    /// Returns sync metadata handles for every scheduled block.
    pub fn block_handles(&self) -> Vec<&BlockHandle> {
        self.handles.values().collect()
    }

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

    pub fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<String>,
    ) -> Result<Uuid> {
        let block_def = get_block(block_name.as_str(), lib.as_deref()).ok_or_else(|| {
            RegistryError::BlockNotFound {
                library: lib.as_deref().unwrap_or(CORE_LIB).to_string(),
                name: block_name.clone(),
            }
        })?;
        schedule_block_on_engine_mt(&block_def.desc, block_id, self)
    }

    /// Sync configuration-time link validation. Real wiring is deferred to
    /// `run()` start (mailbox round-trips need the worker tasks running).
    pub(super) fn connect_blocks_sync(&mut self, link_data: &LinkData) -> Result<LinkData> {
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

    pub async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, EngineError> {
        let mailbox = self.mailbox_or_err(id)?;

        mailbox_request(mailbox, *id, |reply| BlockMailboxCmd::Inspect { reply }).await
    }

    pub async fn write_input(
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

    pub async fn write_output(
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

    pub async fn connect_blocks(&self, link_data: &LinkData) -> Result<LinkData> {
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

        // Source pin: output or input?
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

        // Increment connection count on target — without this,
        // `drain_ready_inputs` skips the input. The new count itself is of
        // no interest, so only the delivery half is checked.
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

        // Seed the target input with the source's current value.
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
        // Refresh another connected input on the target so it re-cycles.
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

    /// MT mirror of the ST helper — see its doc for the
    /// `is_connected` filter rationale.
    async fn reset_connected_inputs(&self, target_id: &Uuid, ignore_input: &str) -> Result<()> {
        let inputs = match self.inspect_block(target_id).await {
            Ok(def) => def.inputs,
            Err(_) => return Ok(()),
        };
        if let Some((name, _data)) = inputs
            .iter()
            .find(|(name, data)| name.as_str() != ignore_input && data.is_connected)
            && let Some(mb) = self.mailbox(target_id)
        {
            let _ = mb
                .send(BlockMailboxCmd::RefreshInput { name: name.clone() })
                .await;
        }
        Ok(())
    }

    pub async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        let target_mb = self.mailbox_or_err(block_id)?.clone();

        // 1. DisconnectAll on the target to learn what to decrement.
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

        // 2. Broadcast RemoveTargetBlockLinks to every other block.
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

    /// MT mirror of [`super::super::single_threaded::engine::SingleThreadedEngine::save_program`].
    pub async fn save_program(&self) -> Result<Program> {
        let mut blocks = std::collections::BTreeMap::new();
        let mut links = std::collections::BTreeMap::new();

        for (id, handle) in &self.handles {
            let definition = mailbox_request(&handle.mailbox, *id, |reply| {
                BlockMailboxCmd::Inspect { reply }
            })
            .await?;

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

    /// MT mirror of [`super::super::single_threaded::engine::SingleThreadedEngine::load_program`].
    pub async fn load_program(&mut self, program: Program) -> Result<()> {
        self.schedule_program_blocks(&program)?;

        let pending_links = std::mem::take(&mut self.pending_links);
        for link in pending_links {
            self.connect_blocks(&link).await?;
        }

        for (uuid_str, pb) in &program.blocks {
            let id = parse_block_uuid(uuid_str)?;
            for (name, pin) in &pb.inputs {
                if hasinitialvalue_mt(&pin.value) {
                    let _ = self.write_input(&id, name.clone(), pin.value.clone()).await;
                }
            }
            for (name, pin) in &pb.outputs {
                if hasinitialvalue_mt(&pin.value) {
                    let _ = self
                        .write_output(&id, name.clone(), pin.value.clone())
                        .await;
                }
            }
        }

        Ok(())
    }

    pub async fn disconnect_link_by_id(&self, link_id: &Uuid) -> Result<bool> {
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

    fn reply_to_sender(&self, sender_uuid: Uuid, engine_message: Messages) {
        for (sender_id, sender) in &self.reply_senders {
            if sender_id != &sender_uuid {
                continue;
            }
            let _ = sender.try_send(engine_message.clone());
        }
    }

    async fn dispatch_message(&mut self, msg: Messages) {
        match msg {
            EngineMessage::AddBlockReq(sender_uuid, block_name, block_uuid, lib) => {
                let block_id = match block_uuid.as_deref().map(parse_block_uuid).transpose() {
                    Ok(id) => id,
                    Err(err) => {
                        return self.reply_to_sender(
                            sender_uuid,
                            EngineMessage::AddBlockRes(Err(err.to_string())),
                        );
                    }
                };
                let res = self
                    .add_block(block_name, block_id, lib)
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::AddBlockRes(res));
            }

            EngineMessage::RemoveBlockReq(sender_uuid, block_id) => {
                let res = self
                    .remove_block(&block_id)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::RemoveBlockRes(res));
            }

            EngineMessage::InspectBlockReq(sender_uuid, block_id) => {
                let res = self
                    .inspect_block(&block_id)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::InspectBlockRes(res));
            }

            EngineMessage::EvaluateBlockReq(sender_uuid, name, inputs, lib) => {
                let Some(block) = get_block(name.as_str(), lib.as_deref()) else {
                    let err = RegistryError::BlockNotFound {
                        library: lib.as_deref().unwrap_or(CORE_LIB).to_string(),
                        name,
                    };
                    return self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::EvaluateBlockRes(Err(err.to_string())),
                    );
                };
                let response = crate::tokio_impl::engine::eval_block(&block.desc, inputs).await;
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::EvaluateBlockRes(response.map_err(|err| err.to_string())),
                );
            }

            EngineMessage::WriteBlockOutputReq(sender_uuid, id, output_name, value) => {
                let res = self
                    .write_output(&id, output_name, value)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::WriteBlockOutputRes(res));
            }

            EngineMessage::WriteBlockInputReq(sender_uuid, id, input_name, value) => {
                let res = self
                    .write_input(&id, input_name, value)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::WriteBlockInputRes(res));
            }

            EngineMessage::WatchBlockSubReq(sender_uuid, sender) => {
                self.watchers.write().await.insert(sender_uuid, sender);
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::WatchBlockSubRes(Ok(sender_uuid)),
                );
            }

            EngineMessage::WatchBlockUnsubReq(sender_uuid) => {
                self.watchers.write().await.remove(&sender_uuid);
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::WatchBlockUnsubRes(Ok(sender_uuid)),
                );
            }

            EngineMessage::GetCurrentProgramReq(sender_uuid) => {
                let res = self.save_program().await.map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::GetCurrentProgramRes(res));
            }

            EngineMessage::LoadProgramReq(sender_uuid, program) => {
                let res = self
                    .load_program(program)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::LoadProgramRes(res));
            }

            EngineMessage::ConnectBlocksReq(sender_uuid, link_data) => {
                let res = self
                    .connect_blocks(&link_data)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::ConnectBlocksRes(res));
            }

            EngineMessage::RemoveLinkReq(sender_uuid, link_id) => {
                let res = self
                    .disconnect_link_by_id(&link_id)
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::RemoveLinkRes(res));
            }

            _ => unreachable!("Invalid message"),
        }
    }
}

/// MT-side mirror of the ST `hasinitialvalue` helper.
fn hasinitialvalue_mt(value: &Value) -> bool {
    use libhaystack::val::Value::*;
    !matches!(value, Null)
        && !matches!(value, Dict(d) if d.is_empty())
        && !matches!(value, List(l) if l.is_empty())
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::base;
    use crate::base::program::data::LinkData;
    use crate::blocks::{math::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes, Shutdown};

    use super::MultiThreadedEngine;
    use base::engine::Engine;
    use tokio::sync::mpsc;
    use tokio::time::sleep;
    use uuid::Uuid;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn multi_threaded_engine_test() {
        use crate::base::block::connect::connect_output;

        let mut add1 = Add::new();
        let add_uuid = *add1.id();

        let mut sine1 = SineWave::new();
        sine1.amplitude.val = Some(3.into());
        sine1.freq.val = Some(200.into());
        connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

        let mut sine2 = SineWave::new();
        sine2.amplitude.val = Some(7.into());
        sine2.freq.val = Some(400.into());
        sine2
            .connect_output("out", add1.inputs_mut()[1])
            .expect("Connected");

        let mut eng = MultiThreadedEngine::new();

        let (sender, mut receiver) = mpsc::channel(32);
        let channel_id = Uuid::new_v4();
        let engine_sender = eng.create_message_channel(channel_id, sender.clone());

        tokio::spawn(async move {
            sleep(Duration::from_millis(300)).await;

            let _ = engine_sender
                .send(InspectBlockReq(channel_id, add_uuid))
                .await;

            let res = receiver.recv().await;

            if let Some(InspectBlockRes(Ok(data))) = res {
                assert_eq!(data.id, add_uuid.to_string());
                assert_eq!(data.name, "Add");
                assert_eq!(data.inputs.len(), 16);
                assert_eq!(data.outputs.len(), 1);
            } else {
                panic!("Failed to find block: {:?}", res)
            }

            let _ = engine_sender.send(Shutdown).await;
        });

        eng.schedule_send(add1);
        eng.schedule_send(sine1);
        eng.schedule_send(sine2);

        eng.run().await;
    }

    /// Regression test for the reported "Bar still receives values after
    /// link delete" bug. Verifies at the engine level that:
    ///   1. A link can be created via the async API.
    ///   2. Writing the source's output reaches the target's cached input.
    ///   3. After `disconnect_link_by_id`, the target stops seeing new
    ///      values from the source (the source's `output.links` no longer
    ///      contains a sender to the target's watch channel) AND the
    ///      target's input `is_connected` flips to false.
    ///
    /// If this test passes, the engine-level disconnect is correct and the
    /// reported UI symptom must be coming from the JS side (most likely
    /// `removeLink` being called with the wrong / missing link id).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn disconnect_link_severs_dataflow() {
        let mut eng = MultiThreadedEngine::new();

        let block_a = Add::new();
        let a_uuid = *block_a.id();
        let block_b = Add::new();
        let b_uuid = *block_b.id();

        eng.schedule_send(block_a);
        eng.schedule_send(block_b);

        // Wire A.out -> B.in0 via the engine's async API (same path the
        // wasm bridge takes for `createLink`).
        let link_request = LinkData {
            id: None,
            source_block_uuid: a_uuid.to_string(),
            target_block_uuid: b_uuid.to_string(),
            source_block_pin_name: "out".to_string(),
            target_block_pin_name: "in0".to_string(),
        };
        let link_data = eng
            .connect_blocks(&link_request)
            .await
            .expect("connect_blocks");
        let link_id =
            Uuid::parse_str(link_data.id.as_ref().expect("link id")).expect("link id is uuid");

        // Push 42 through A.out. Wait long enough for A's actor to handle
        // the WriteOutput mailbox cmd (pushes to A.out.links -> B.in0
        // watch channel) and for B's actor to drain its input on the next
        // execute cycle.
        let _ = eng
            .write_output(&a_uuid, "out".to_string(), 42.into())
            .await;
        sleep(Duration::from_millis(150)).await;

        let snap = eng.inspect_block(&b_uuid).await.expect("inspect B");
        let in0_before = snap.inputs.get("in0").expect("B has in0");
        assert_eq!(
            in0_before.val,
            42.into(),
            "B.in0 should see 42 before disconnect, got {:?}",
            in0_before.val
        );
        assert!(
            in0_before.is_connected,
            "B.in0 should report is_connected=true before disconnect"
        );

        // Disconnect.
        let removed = eng
            .disconnect_link_by_id(&link_id)
            .await
            .expect("disconnect_link_by_id");
        assert!(
            removed,
            "disconnect_link_by_id should report it removed the link"
        );

        // Push a new value through A.out. If the link is properly severed,
        // A.out.links no longer contains a sender to B's channel, so the
        // value never reaches B and B.in0.val stays at 42.
        let _ = eng
            .write_output(&a_uuid, "out".to_string(), 99.into())
            .await;
        sleep(Duration::from_millis(150)).await;

        let snap = eng.inspect_block(&b_uuid).await.expect("inspect B after");
        let in0_after = snap.inputs.get("in0").expect("B has in0");
        assert_ne!(
            in0_after.val,
            99.into(),
            "after disconnect, B.in0 should NOT have received 99; got {:?}",
            in0_after.val
        );
        assert!(
            !in0_after.is_connected,
            "after disconnect, B.in0 should report is_connected=false"
        );
    }
}
