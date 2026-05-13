// Copyright (c) 2022-2026, Radu Racariu.

//! Multi-threaded engine.
//!
//! Distributes blocks across worker threads in round-robin. Each worker
//! runs its own current-thread tokio runtime hosting a `LocalSet`; block
//! actor tasks live on those `LocalSet`s. The engine itself runs on
//! whichever runtime called `run()` (typically the multi-threaded runtime
//! the user chose at startup).
//!
//! Shares the per-block mailbox protocol ([`super::super::block_mailbox`])
//! with the single-threaded engine, so the actor task implementation is
//! near-identical — the only MT-specific bits are watchers
//! (`Arc<RwLock<...>>`) and the worker-thread plumbing.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;

use anyhow::{Result, anyhow};
use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, Receiver, Sender, UnboundedSender},
    oneshot,
};
use tokio::task::LocalSet;
use uuid::Uuid;

use super::super::block_mailbox::{BLOCK_MAILBOX_CAP, BlockMailboxCmd};
use super::actor::{WatchersHandle, block_actor_task};
use crate::base::{
    block::{Block, BlockDesc},
    engine::messages::{BlockDefinition, EngineMessage, WatchMessage},
    program::data::{BlockData, LinkData},
};
use crate::blocks::registry::get_block;
use crate::tokio_impl::engine::schedule_block_on_engine_mt;
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Concrete engine-message type.
///
/// The watch-event sender is unbounded — see `wasm/engine_command.rs`
/// `create_watch` for the rationale.
pub type Messages = EngineMessage<UnboundedSender<WatchMessage>>;

/// Engine-side handle for a scheduled block in the MT engine.
///
/// In addition to the usual id/name/lib/desc + mailbox sender, MT handles
/// remember which worker thread the block lives on (round-robin assigned
/// at scheduling time).
pub struct BlockHandle {
    id: Uuid,
    name: String,
    library: String,
    desc: &'static BlockDesc,
    worker_idx: usize,
    mailbox: mpsc::Sender<BlockMailboxCmd>,
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
    pub fn desc(&self) -> &'static BlockDesc {
        self.desc
    }
    pub fn worker_idx(&self) -> usize {
        self.worker_idx
    }
}

/// Commands sent from the engine to a worker thread.
enum WorkerCommand {
    /// Spawn a per-block actor task on the worker's `LocalSet`. The boxed
    /// closure captures the block, its mailbox receiver, and a clone of
    /// the watchers handle; on the worker side it spawns the actor task.
    Schedule(Box<dyn FnOnce(&mut WorkerState) + Send>),
    /// Worker should drain its `LocalSet` (best-effort) and exit.
    Shutdown,
}

struct WorkerState {
    local: LocalSet,
}

/// Multi-threaded execution environment for blocks.
pub struct MultiThreadedEngine {
    workers: Vec<Sender<WorkerCommand>>,
    handles: BTreeMap<Uuid, BlockHandle>,
    pending_links: Vec<LinkData>,
    next_worker: usize,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
    pub(in super::super) reply_senders: BTreeMap<Uuid, Sender<Messages>>,
    pub(in super::super) watchers: WatchersHandle,
}

impl Default for MultiThreadedEngine {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

impl crate::base::engine::Engine for MultiThreadedEngine {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type Channel = Sender<Messages>;

    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        _block: B,
    ) {
        // The MT engine requires `Send` because the block crosses the
        // worker-thread boundary. Use [`MultiThreadedEngine::schedule_send`]
        // for the Send-bounded entry point; this trait method is here only
        // to satisfy the `Engine` contract and panics if called.
        panic!(
            "MultiThreadedEngine::schedule (trait) requires `Send`; \
             use the inherent `schedule_send` method instead"
        );
    }

    fn load_blocks_and_links(&mut self, blocks: &[BlockData], links: &[LinkData]) -> Result<()> {
        for block in blocks {
            let id = Uuid::try_from(block.id.as_str()).map_err(|_| anyhow!("Invalid block id"))?;
            let block_def = get_block(&block.name, Some(block.lib.clone()))
                .ok_or_else(|| anyhow!("Block not found"))?;
            schedule_block_on_engine_mt(&block_def.desc, Some(id), self)?;
        }
        for link in links {
            self.connect_blocks_sync(link)?;
        }
        Ok(())
    }

    async fn run(&mut self) {
        // Process any links queued during configuration via the async
        // mailbox path (worker tasks are already running).
        let pending_links = std::mem::take(&mut self.pending_links);
        for link in pending_links {
            let _ = self.connect_blocks(&link).await;
        }

        let mut is_paused = false;
        loop {
            let engine_msg = self.receiver.recv().await;
            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    for worker in &self.workers {
                        let _ = worker.send(WorkerCommand::Shutdown).await;
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
    /// Create a new multi-threaded engine with the given number of worker
    /// threads. Each worker spins up its own current-thread tokio runtime.
    pub fn new(num_workers: usize) -> Self {
        let num_workers = num_workers.max(1);
        let (sender, receiver) = mpsc::channel(32);
        let mut workers = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>(64);
            workers.push(cmd_tx);

            thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create worker runtime");
                rt.block_on(worker_loop(cmd_rx));
            });
        }

        Self {
            workers,
            handles: BTreeMap::new(),
            pending_links: Vec::new(),
            next_worker: 0,
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Schedule a block on the engine. The block must be `Send` so it can
    /// be moved to a worker thread; once on the worker, the actor task's
    /// future stays put on that worker's `LocalSet` and does not need to be
    /// `Send`.
    pub fn schedule_send<B>(&mut self, block: B)
    where
        B: Block<Writer = WriterImpl, Reader = ReaderImpl> + Send + 'static,
    {
        let worker_idx = self.next_worker % self.workers.len();
        self.next_worker += 1;

        let id = *block.id();
        let name = block.name().to_string();
        let library = block.desc().library.clone();
        let desc: &'static BlockDesc = block.desc();
        let (mailbox_tx, mailbox_rx) = mpsc::channel::<BlockMailboxCmd>(BLOCK_MAILBOX_CAP);

        let handle = BlockHandle {
            id,
            name,
            library,
            desc,
            worker_idx,
            mailbox: mailbox_tx,
        };
        self.handles.insert(id, handle);

        let watchers = self.watchers.clone();
        let spawn_fn: Box<dyn FnOnce(&mut WorkerState) + Send> =
            Box::new(move |state: &mut WorkerState| {
                state
                    .local
                    .spawn_local(block_actor_task(block, mailbox_rx, watchers));
            });

        // Best-effort: if the worker is full or gone, the block is silently
        // dropped. Mirrors the previous behaviour of this engine.
        let _ = self.workers[worker_idx].try_send(WorkerCommand::Schedule(spawn_fn));
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

    pub fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<String>,
    ) -> Result<Uuid> {
        let block_def =
            get_block(block_name.as_str(), lib).ok_or_else(|| anyhow!("Block not found"))?;
        schedule_block_on_engine_mt(&block_def.desc, block_id, self)
    }

    /// Sync configuration-time link validation. Real wiring is deferred to
    /// `run()` start (mailbox round-trips need the worker tasks running).
    pub(super) fn connect_blocks_sync(&mut self, link_data: &LinkData) -> Result<LinkData> {
        let source_id = Uuid::try_from(link_data.source_block_uuid.as_str())?;
        let target_id = Uuid::try_from(link_data.target_block_uuid.as_str())?;
        let source_handle = self
            .block_handle(&source_id)
            .ok_or_else(|| anyhow!("Source block '{}' not found", link_data.source_block_uuid))?;
        let target_handle = self
            .block_handle(&target_id)
            .ok_or_else(|| anyhow!("Target block '{}' not found", link_data.target_block_uuid))?;

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
            return Err(anyhow!(
                "Source pin '{}' not found on block '{}'",
                source_pin,
                link_data.source_block_uuid
            ));
        }

        let target_pin = link_data.target_block_pin_name.as_str();
        let target_pin_exists = target_handle
            .desc()
            .inputs
            .iter()
            .any(|i| i.name == target_pin);
        if !target_pin_exists {
            return Err(anyhow!(
                "Target input '{}' not found on block '{}'",
                target_pin,
                link_data.target_block_uuid
            ));
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

    pub async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, String> {
        let mailbox = self
            .mailbox(id)
            .ok_or_else(|| "Block not found".to_string())?;
        let (tx, rx) = oneshot::channel();
        mailbox
            .send(BlockMailboxCmd::Inspect { reply: tx })
            .await
            .map_err(|_| "Block task gone".to_string())?;
        rx.await.map_err(|_| "Block task dropped reply".to_string())
    }

    pub async fn write_input(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Option<Value>, String> {
        let mailbox = self
            .mailbox(id)
            .ok_or_else(|| "Block not found".to_string())?;
        let (tx, rx) = oneshot::channel();
        mailbox
            .send(BlockMailboxCmd::WriteInput {
                name,
                value,
                reply: tx,
            })
            .await
            .map_err(|_| "Block task gone".to_string())?;
        rx.await
            .map_err(|_| "Block task dropped reply".to_string())?
    }

    pub async fn write_output(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Value, String> {
        let mailbox = self
            .mailbox(id)
            .ok_or_else(|| "Block not found".to_string())?;
        let (tx, rx) = oneshot::channel();
        mailbox
            .send(BlockMailboxCmd::WriteOutput {
                name,
                value,
                reply: tx,
            })
            .await
            .map_err(|_| "Block task gone".to_string())?;
        rx.await
            .map_err(|_| "Block task dropped reply".to_string())?
    }

    pub async fn connect_blocks(&self, link_data: &LinkData) -> Result<LinkData> {
        let source_id = Uuid::try_from(link_data.source_block_uuid.as_str())?;
        let target_id = Uuid::try_from(link_data.target_block_uuid.as_str())?;

        let source_mb = self
            .mailbox(&source_id)
            .ok_or_else(|| anyhow!("Source block '{}' not found", link_data.source_block_uuid))?;
        let target_mb = self
            .mailbox(&target_id)
            .ok_or_else(|| anyhow!("Target block '{}' not found", link_data.target_block_uuid))?;

        // Get the writer of the target input.
        let (tx, rx) = oneshot::channel();
        target_mb
            .send(BlockMailboxCmd::GetInputWriter {
                name: link_data.target_block_pin_name.clone(),
                reply: tx,
            })
            .await
            .map_err(|_| anyhow!("Target block task gone"))?;
        let target_writer = rx
            .await
            .map_err(|_| anyhow!("Target block dropped reply"))?
            .map_err(|e| anyhow!(e))?;

        // Source pin: output or input?
        let (has_tx, has_rx) = oneshot::channel();
        source_mb
            .send(BlockMailboxCmd::HasOutput {
                name: link_data.source_block_pin_name.clone(),
                reply: has_tx,
            })
            .await
            .map_err(|_| anyhow!("Source block task gone"))?;
        let is_output = has_rx
            .await
            .map_err(|_| anyhow!("Source block dropped reply"))?;

        let (link_tx, link_rx) = oneshot::channel();
        if is_output {
            source_mb
                .send(BlockMailboxCmd::AddOutputLink {
                    output_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_id,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    target_writer: target_writer.clone(),
                    reply: link_tx,
                })
                .await
                .map_err(|_| anyhow!("Source block task gone"))?;
        } else {
            source_mb
                .send(BlockMailboxCmd::AddInputLink {
                    input_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_id,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    target_writer: target_writer.clone(),
                    reply: link_tx,
                })
                .await
                .map_err(|_| anyhow!("Source block task gone"))?;
        }
        let link_id = link_rx
            .await
            .map_err(|_| anyhow!("Source block dropped reply"))?
            .map_err(|e| anyhow!(e))?;

        // Increment connection count on target — without this,
        // `drain_ready_inputs` skips the input.
        let (inc_tx, inc_rx) = oneshot::channel();
        target_mb
            .send(BlockMailboxCmd::IncrementInput {
                name: link_data.target_block_pin_name.clone(),
                reply: inc_tx,
            })
            .await
            .map_err(|_| anyhow!("Target block task gone"))?;
        let _ = inc_rx.await;

        // Seed the target input with the source's current value.
        target_mb
            .send(BlockMailboxCmd::SeedInputValue {
                name: link_data.target_block_pin_name.clone(),
                value: self
                    .read_source_value(&source_id, &link_data.source_block_pin_name, is_output)
                    .await
                    .unwrap_or_default(),
            })
            .await
            .map_err(|_| anyhow!("Target block task gone"))?;
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

    async fn reset_connected_inputs(&self, target_id: &Uuid, ignore_input: &str) -> Result<()> {
        let inputs = match self.inspect_block(target_id).await {
            Ok(def) => def.inputs,
            Err(_) => return Ok(()),
        };
        if let Some((name, _data)) = inputs
            .iter()
            .find(|(name, _)| name.as_str() != ignore_input)
            && let Some(mb) = self.mailbox(target_id)
        {
            let _ = mb
                .send(BlockMailboxCmd::RefreshInput { name: name.clone() })
                .await;
        }
        Ok(())
    }

    pub async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        let target_mb = self
            .mailbox(block_id)
            .ok_or_else(|| anyhow!("Block not found"))?
            .clone();

        // 1. DisconnectAll on the target to learn what to decrement.
        let (tx, rx) = oneshot::channel();
        target_mb
            .send(BlockMailboxCmd::DisconnectAll { reply: tx })
            .await
            .map_err(|_| anyhow!("Block task gone"))?;
        let targets = rx.await.map_err(|_| anyhow!("Block dropped reply"))?;
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

    pub async fn save_blocks_and_links(&self) -> Result<(Vec<BlockData>, Vec<LinkData>)> {
        let mut blocks = Vec::new();
        let mut links = Vec::new();
        for handle in self.handles.values() {
            let (tx, rx) = oneshot::channel();
            handle
                .mailbox
                .send(BlockMailboxCmd::GetBlockData { reply: tx })
                .await
                .map_err(|_| anyhow!("Block task gone"))?;
            let (block_data, block_links) = rx.await.map_err(|_| anyhow!("Block dropped reply"))?;
            blocks.push(block_data);
            links.extend(block_links);
        }
        Ok((blocks, links))
    }

    pub async fn disconnect_link_by_id(&self, link_id: &Uuid) -> Result<bool> {
        for handle in self.handles.values() {
            let (tx, rx) = oneshot::channel();
            if handle
                .mailbox
                .send(BlockMailboxCmd::DisconnectLink {
                    link_id: *link_id,
                    reply: tx,
                })
                .await
                .is_err()
            {
                continue;
            }
            let targets = rx.await.map_err(|_| anyhow!("Block dropped reply"))?;
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
                let block_id = if let Some(uuid) = block_uuid {
                    match Uuid::parse_str(&uuid) {
                        Ok(uuid) => Some(uuid),
                        Err(_) => {
                            return self.reply_to_sender(
                                sender_uuid,
                                EngineMessage::AddBlockRes(Err("Invalid UUID".into())),
                            );
                        }
                    }
                } else {
                    None
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
                let res = self.inspect_block(&block_id).await;
                self.reply_to_sender(sender_uuid, EngineMessage::InspectBlockRes(res));
            }

            EngineMessage::EvaluateBlockReq(sender_uuid, name, inputs, lib) => {
                let Some(block) = get_block(name.as_str(), lib) else {
                    return self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::EvaluateBlockRes(Err("Block not found".into())),
                    );
                };
                let response = crate::tokio_impl::engine::eval_block(&block.desc, inputs).await;
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::EvaluateBlockRes(response.map_err(|err| err.to_string())),
                );
            }

            EngineMessage::WriteBlockOutputReq(sender_uuid, id, output_name, value) => {
                let res = self.write_output(&id, output_name, value).await;
                self.reply_to_sender(sender_uuid, EngineMessage::WriteBlockOutputRes(res));
            }

            EngineMessage::WriteBlockInputReq(sender_uuid, id, input_name, value) => {
                let res = self.write_input(&id, input_name, value).await;
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
                let res = self
                    .save_blocks_and_links()
                    .await
                    .map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::GetCurrentProgramRes(res));
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

async fn worker_loop(mut cmd_rx: Receiver<WorkerCommand>) {
    let mut state = WorkerState {
        local: LocalSet::new(),
    };

    loop {
        let mut cmd = None;
        // Drive local tasks while waiting for the next worker command.
        state
            .local
            .run_until(async {
                cmd = cmd_rx.recv().await;
            })
            .await;

        match cmd {
            Some(WorkerCommand::Schedule(spawn_fn)) => spawn_fn(&mut state),
            Some(WorkerCommand::Shutdown) | None => break,
        }
    }
}

fn num_cpus() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::base;
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

        let mut eng = MultiThreadedEngine::new(2);

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
}
