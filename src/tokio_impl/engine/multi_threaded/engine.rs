// Copyright (c) 2022-2025, Radu Racariu.

//!
//! Multi-threaded engine implementation.
//!
//! Distributes blocks across worker threads, each running a single-threaded
//! `LocalSet` executor. This provides true parallelism while keeping the
//! block execution model identical to the single-threaded engine.
//!
//! Communication between the engine and workers uses channels,
//! so no shared mutable state exists across threads.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::thread;

use anyhow::{Result, anyhow};
use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, Receiver, Sender},
    oneshot,
};
use tokio::task::LocalSet;
use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockProps, BlockState},
        engine::messages::{
            BlockDefinition, BlockInputData, BlockOutputData, ChangeSource, EngineMessage,
            WatchMessage,
        },
        program::data::{BlockData, LinkData},
    },
    blocks::registry::get_block,
    tokio_impl::{ReaderImpl, WriterImpl},
};

use super::messages::BlockCommand;

use crate::tokio_impl::engine::schedule_block_on_engine_mt;

/// The concrete trait for the block properties
type BlockPropsType = dyn BlockProps<Writer = WriterImpl, Reader = ReaderImpl>;

/// The concrete type for the engine messages
pub type Messages = EngineMessage<Sender<WatchMessage>>;

/// A command sent from the engine to a worker thread.
enum WorkerCommand {
    /// Schedule a new block on this worker's LocalSet.
    /// The boxed closure captures the block and spawns it on the LocalSet.
    Schedule(Box<dyn FnOnce(&mut WorkerState) + Send>),
    /// Forward a block command to a specific block on this worker.
    BlockCmd(Uuid, BlockCommand),
    /// Shutdown the worker.
    Shutdown,
}

/// State held by each worker thread.
struct WorkerState {
    local: LocalSet,
    /// Block properties accessed via UnsafeCell (same pattern as single-threaded engine)
    block_props: BTreeMap<Uuid, std::rc::Rc<dyn AnyBlockProps>>,
}

/// Type-erased access to block properties (same as single-threaded engine).
trait AnyBlockProps {
    /// # Safety: caller must ensure no concurrent mutable access.
    #[allow(clippy::mut_from_ref)]
    unsafe fn props_mut(&self) -> &mut BlockPropsType;
}

impl<B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static> AnyBlockProps
    for SharedBlock<B>
{
    unsafe fn props_mut(&self) -> &mut BlockPropsType {
        unsafe { &mut *self.cell.get() }
    }
}

struct SharedBlock<B> {
    cell: std::cell::UnsafeCell<B>,
}

impl<B> SharedBlock<B> {
    fn new(block: B) -> Self {
        Self {
            cell: std::cell::UnsafeCell::new(block),
        }
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut(&self) -> &mut B {
        unsafe { &mut *self.cell.get() }
    }
}

/// Multi-threaded execution environment for blocks.
///
/// Distributes blocks across `N` worker threads in round-robin fashion.
/// Each worker runs its own tokio `LocalSet`, providing the same
/// single-threaded execution model per worker while enabling parallelism
/// across workers.
pub struct MultiThreadedEngine {
    /// Worker command channels
    workers: Vec<Sender<WorkerCommand>>,
    /// Maps block UUID to worker index
    block_worker: BTreeMap<Uuid, usize>,
    /// Round-robin counter for distributing blocks
    next_worker: usize,
    /// Messaging channel used by external processes
    sender: Sender<Messages>,
    /// Multi-producer single-consumer channel for receiving messages
    receiver: Receiver<Messages>,
    /// Senders used to reply to issued commands
    reply_senders: BTreeMap<Uuid, Sender<Messages>>,
    /// Watchers for changes in block pins
    watchers: Arc<RwLock<BTreeMap<Uuid, Sender<WatchMessage>>>>,
}

impl Default for MultiThreadedEngine {
    fn default() -> Self {
        Self::new(num_cpus())
    }
}

impl MultiThreadedEngine {
    /// Create a new multi-threaded engine with the given number of worker threads.
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
            block_worker: BTreeMap::new(),
            next_worker: 0,
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Arc::default(),
        }
    }

    /// Schedule a block for execution. The block must be `Send` to be moved
    /// to a worker thread, but its `execute()` future does not need to be `Send`.
    pub fn schedule<B: Block<Writer = WriterImpl, Reader = ReaderImpl> + Send + 'static>(
        &mut self,
        block: B,
    ) {
        let worker_idx = self.next_worker % self.workers.len();
        self.next_worker += 1;

        let block_id = *block.id();
        self.block_worker.insert(block_id, worker_idx);

        let watchers = self.watchers.clone();

        // Send a closure that will schedule the block on the worker's LocalSet
        let schedule_fn: Box<dyn FnOnce(&mut WorkerState) + Send> =
            Box::new(move |state: &mut WorkerState| {
                let shared = std::rc::Rc::new(SharedBlock::new(block));
                let id = *unsafe { shared.get_mut() }.id();
                state
                    .block_props
                    .insert(id, shared.clone() as std::rc::Rc<dyn AnyBlockProps>);

                let watchers = watchers;
                state.local.spawn_local(async move {
                    let mut last_pin_values = BTreeMap::<String, Value>::new();
                    loop {
                        let block = unsafe { shared.get_mut() };
                        block.execute().await;

                        change_of_value_check(&watchers, block, &mut last_pin_values).await;

                        if block.state() == BlockState::Terminated {
                            break;
                        }
                    }
                });
            });

        let _ = self.workers[worker_idx].try_send(WorkerCommand::Schedule(schedule_fn));
    }

    pub fn load_blocks_and_links(
        &mut self,
        blocks: &[BlockData],
        links: &[LinkData],
    ) -> Result<()> {
        blocks.iter().try_for_each(|block| -> Result<()> {
            let id = Uuid::try_from(block.id.as_str()).ok();
            if id.is_none() {
                return Err(anyhow!("Invalid block id"));
            }

            let block = get_block(&block.name, Some(block.lib.clone()))
                .ok_or_else(|| anyhow!("Block not found"))?;
            schedule_block_on_engine_mt(&block.desc, id, self)?;

            Ok(())
        })?;

        // Connect blocks - send commands to workers
        for link in links {
            self.connect_blocks_sync(link)?;
        }

        Ok(())
    }

    pub async fn run(&mut self) {
        let mut is_paused = false;
        loop {
            let engine_msg = self.receiver.recv().await;

            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    // Shutdown all workers
                    for worker in &self.workers {
                        let _ = worker.send(WorkerCommand::Shutdown).await;
                    }
                    break;
                } else if matches!(message, EngineMessage::Reset) {
                    // Terminate all blocks
                    for (&block_id, &worker_idx) in &self.block_worker {
                        let _ = self.workers[worker_idx]
                            .send(WorkerCommand::BlockCmd(
                                block_id,
                                BlockCommand::SetState(BlockState::Terminated),
                            ))
                            .await;
                    }
                    self.block_worker.clear();
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

    pub fn create_message_channel(
        &mut self,
        sender_id: Uuid,
        sender_channel: Sender<Messages>,
    ) -> Sender<Messages> {
        self.reply_senders.insert(sender_id, sender_channel);
        self.sender.clone()
    }

    pub fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<String>,
    ) -> Result<Uuid> {
        let block =
            get_block(block_name.as_str(), lib).ok_or_else(|| anyhow!("Block not found"))?;
        schedule_block_on_engine_mt(&block.desc, block_id, self)
    }

    pub fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        let worker_idx = self
            .block_worker
            .get(block_id)
            .ok_or_else(|| anyhow!("Block not found"))?;

        let _ = self.workers[*worker_idx].try_send(WorkerCommand::BlockCmd(
            *block_id,
            BlockCommand::SetState(BlockState::Terminated),
        ));

        // Remove links targeting this block from all other blocks
        for (&other_id, &other_worker) in &self.block_worker {
            if &other_id != block_id {
                let _ = self.workers[other_worker].try_send(WorkerCommand::BlockCmd(
                    other_id,
                    BlockCommand::RemoveTargetBlockLinks(*block_id),
                ));
            }
        }

        self.block_worker.remove(block_id);
        Ok(*block_id)
    }

    /// Connect blocks synchronously (used during load_blocks_and_links).
    fn connect_blocks_sync(&self, link_data: &LinkData) -> Result<LinkData> {
        let source_block_uuid = Uuid::try_from(link_data.source_block_uuid.as_str())?;
        let target_block_uuid = Uuid::try_from(link_data.target_block_uuid.as_str())?;

        let target_worker = self
            .block_worker
            .get(&target_block_uuid)
            .ok_or_else(|| anyhow!("Target block not found"))?;
        let source_worker = self
            .block_worker
            .get(&source_block_uuid)
            .ok_or_else(|| anyhow!("Source block not found"))?;

        // Step 1: Get writer from target input
        let (writer_tx, writer_rx) = oneshot::channel();
        self.workers[*target_worker]
            .try_send(WorkerCommand::BlockCmd(
                target_block_uuid,
                BlockCommand::GetInputWriter(link_data.target_block_pin_name.clone(), writer_tx),
            ))
            .map_err(|_| anyhow!("Failed to send GetInputWriter"))?;

        let writer = writer_rx
            .blocking_recv()
            .map_err(|_| anyhow!("Failed to receive writer"))?
            .map_err(|e| anyhow!(e))?;

        // Step 2: Add link on source block
        let (link_tx, link_rx) = oneshot::channel();
        self.workers[*source_worker]
            .try_send(WorkerCommand::BlockCmd(
                source_block_uuid,
                BlockCommand::AddOutputLink {
                    output_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_block_uuid,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    writer,
                    reply: link_tx,
                },
            ))
            .map_err(|_| anyhow!("Failed to send AddOutputLink"))?;

        let link_id = link_rx
            .blocking_recv()
            .map_err(|_| anyhow!("Failed to receive link response"))?
            .map_err(|e| anyhow!(e))?;

        Ok(LinkData {
            id: Some(link_id.to_string()),
            ..link_data.clone()
        })
    }

    /// Connect blocks asynchronously.
    async fn connect_blocks(&self, link_data: &LinkData) -> Result<LinkData> {
        let source_block_uuid = Uuid::try_from(link_data.source_block_uuid.as_str())?;
        let target_block_uuid = Uuid::try_from(link_data.target_block_uuid.as_str())?;

        let target_worker = self
            .block_worker
            .get(&target_block_uuid)
            .ok_or_else(|| anyhow!("Target block not found"))?;
        let source_worker = self
            .block_worker
            .get(&source_block_uuid)
            .ok_or_else(|| anyhow!("Source block not found"))?;

        // Step 1: Get writer from target input
        let (writer_tx, writer_rx) = oneshot::channel();
        self.workers[*target_worker]
            .send(WorkerCommand::BlockCmd(
                target_block_uuid,
                BlockCommand::GetInputWriter(link_data.target_block_pin_name.clone(), writer_tx),
            ))
            .await
            .map_err(|_| anyhow!("Failed to send GetInputWriter"))?;

        let writer = writer_rx
            .await
            .map_err(|_| anyhow!("Failed to receive writer"))?
            .map_err(|e| anyhow!(e))?;

        // Step 2: Add link on source block
        let (link_tx, link_rx) = oneshot::channel();
        self.workers[*source_worker]
            .send(WorkerCommand::BlockCmd(
                source_block_uuid,
                BlockCommand::AddOutputLink {
                    output_name: link_data.source_block_pin_name.clone(),
                    target_block_id: target_block_uuid,
                    target_input_name: link_data.target_block_pin_name.clone(),
                    writer,
                    reply: link_tx,
                },
            ))
            .await
            .map_err(|_| anyhow!("Failed to send AddOutputLink"))?;

        let link_id = link_rx
            .await
            .map_err(|_| anyhow!("Failed to receive link response"))?
            .map_err(|e| anyhow!(e))?;

        Ok(LinkData {
            id: Some(link_id.to_string()),
            ..link_data.clone()
        })
    }

    async fn save_blocks_and_links(&self) -> Result<(Vec<BlockData>, Vec<LinkData>)> {
        let mut blocks = Vec::new();
        let mut links = Vec::new();

        for (&block_id, &worker_idx) in &self.block_worker {
            let (tx, rx) = oneshot::channel();
            if self.workers[worker_idx]
                .send(WorkerCommand::BlockCmd(
                    block_id,
                    BlockCommand::GetBlockData(tx),
                ))
                .await
                .is_ok()
                && let Ok((block_data, block_links)) = rx.await
            {
                blocks.push(block_data);
                links.extend(block_links);
            }
        }

        Ok((blocks, links))
    }

    async fn dispatch_message(&mut self, msg: Messages) {
        match msg {
            EngineMessage::AddBlockReq(sender_uuid, block_name, block_uuid, lib) => {
                log::debug!(
                    "Adding block: {}::{}",
                    lib.clone().unwrap_or("core".into()),
                    block_name,
                );

                let block_id = if let Some(uuid) = block_uuid {
                    match Uuid::parse_str(&uuid) {
                        Ok(uuid) => Some(uuid),
                        Err(_) => {
                            self.reply_to_sender(
                                sender_uuid,
                                EngineMessage::AddBlockRes(Err("Invalid UUID".into())),
                            );
                            return;
                        }
                    }
                } else {
                    None
                };

                let block_id = self
                    .add_block(block_name, block_id, lib)
                    .map_err(|err| err.to_string());

                self.reply_to_sender(sender_uuid, EngineMessage::AddBlockRes(block_id));
            }

            EngineMessage::RemoveBlockReq(sender_uuid, block_id) => {
                let block_id = self.remove_block(&block_id).map_err(|err| err.to_string());
                self.reply_to_sender(sender_uuid, EngineMessage::RemoveBlockRes(block_id));
            }

            EngineMessage::InspectBlockReq(sender_uuid, block_uuid) => {
                let response = if let Some(&worker_idx) = self.block_worker.get(&block_uuid) {
                    let (tx, rx) = oneshot::channel();
                    if self.workers[worker_idx]
                        .send(WorkerCommand::BlockCmd(
                            block_uuid,
                            BlockCommand::Inspect(tx),
                        ))
                        .await
                        .is_ok()
                    {
                        rx.await.unwrap_or(Err("Worker terminated".into()))
                    } else {
                        Err("Worker not available".into())
                    }
                } else {
                    Err("Block not found".into())
                };

                self.reply_to_sender(sender_uuid, EngineMessage::InspectBlockRes(response));
            }

            EngineMessage::EvaluateBlockReq(sender_uuid, name, inputs, lib) => {
                let Some(block) = get_block(name.as_str(), lib) else {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::EvaluateBlockRes(Err("Block not found".into())),
                    );
                    return;
                };

                let response = crate::tokio_impl::engine::eval_block(&block.desc, inputs).await;

                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::EvaluateBlockRes(response.map_err(|err| err.to_string())),
                );
            }

            EngineMessage::WriteBlockOutputReq(sender_uuid, block_uuid, output_name, value) => {
                let response = if let Some(&worker_idx) = self.block_worker.get(&block_uuid) {
                    let (tx, rx) = oneshot::channel();
                    if self.workers[worker_idx]
                        .send(WorkerCommand::BlockCmd(
                            block_uuid,
                            BlockCommand::WriteOutput(output_name, value, tx),
                        ))
                        .await
                        .is_ok()
                    {
                        rx.await.unwrap_or(Err("Worker terminated".into()))
                    } else {
                        Err("Worker not available".into())
                    }
                } else {
                    Err("Block not found".into())
                };

                self.reply_to_sender(sender_uuid, EngineMessage::WriteBlockOutputRes(response));
            }

            EngineMessage::WriteBlockInputReq(sender_uuid, block_uuid, input_name, value) => {
                let response = if let Some(&worker_idx) = self.block_worker.get(&block_uuid) {
                    let (tx, rx) = oneshot::channel();
                    if self.workers[worker_idx]
                        .send(WorkerCommand::BlockCmd(
                            block_uuid,
                            BlockCommand::WriteInput(input_name, value, tx),
                        ))
                        .await
                        .is_ok()
                    {
                        rx.await.unwrap_or(Err("Worker terminated".into()))
                    } else {
                        Err("Worker not available".into())
                    }
                } else {
                    Err("Block not found".into())
                };

                self.reply_to_sender(sender_uuid, EngineMessage::WriteBlockInputRes(response));
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
                let program = self.save_blocks_and_links().await;
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::GetCurrentProgramRes(program.map_err(|err| err.to_string())),
                );
            }

            EngineMessage::ConnectBlocksReq(sender_uuid, link_data) => {
                let res = self.connect_blocks(&link_data).await;
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::ConnectBlocksRes(res.map_err(|err| err.to_string())),
                );
            }

            EngineMessage::RemoveLinkReq(sender_uuid, link_id) => {
                let mut found = false;
                for (&block_id, &worker_idx) in &self.block_worker {
                    let (tx, rx) = oneshot::channel();
                    if self.workers[worker_idx]
                        .send(WorkerCommand::BlockCmd(
                            block_id,
                            BlockCommand::RemoveLink(link_id, tx),
                        ))
                        .await
                        .is_ok()
                        && let Ok(true) = rx.await
                    {
                        found = true;
                        break;
                    }
                }
                self.reply_to_sender(sender_uuid, EngineMessage::RemoveLinkRes(Ok(found)));
            }

            _ => unreachable!("Invalid message"),
        }
    }

    fn reply_to_sender(&self, sender_uuid: Uuid, engine_message: Messages) {
        for (sender_id, sender) in &self.reply_senders {
            if sender_id != &sender_uuid {
                continue;
            }
            let _ = sender.try_send(engine_message.clone());
        }
    }
}

/// The worker loop runs on a dedicated thread with its own current-thread tokio runtime.
async fn worker_loop(mut cmd_rx: Receiver<WorkerCommand>) {
    let mut state = WorkerState {
        local: tokio::task::LocalSet::new(),
        block_props: BTreeMap::new(),
    };

    loop {
        let mut cmd = None;

        // Run local tasks while waiting for commands
        state
            .local
            .run_until(async {
                cmd = cmd_rx.recv().await;
            })
            .await;

        match cmd {
            Some(WorkerCommand::Schedule(schedule_fn)) => {
                schedule_fn(&mut state);
            }
            Some(WorkerCommand::BlockCmd(block_id, block_cmd)) => {
                if let Some(shared) = state.block_props.get(&block_id) {
                    // SAFETY: Single-threaded LocalSet, no concurrent access.
                    let block = unsafe { shared.props_mut() };
                    handle_block_command(block, block_cmd);
                } else {
                    // Block not found - send error responses for commands that expect replies
                    send_error_reply(block_cmd);
                }
            }
            Some(WorkerCommand::Shutdown) => {
                // Terminate all blocks on this worker
                for shared in state.block_props.values() {
                    let block = unsafe { shared.props_mut() };
                    block.set_state(BlockState::Terminated);
                }
                break;
            }
            None => break,
        }
    }
}

/// Handle a block command on the worker thread.
fn handle_block_command(block: &mut BlockPropsType, cmd: BlockCommand) {
    match cmd {
        BlockCommand::Inspect(reply) => {
            let data = BlockDefinition {
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
            };
            let _ = reply.send(Ok(data));
        }

        BlockCommand::WriteOutput(output_name, value, reply) => {
            let response = if let Some(output) = block.get_output_mut(&output_name) {
                let prev = output.value().clone();
                output.set(value);
                Ok(prev)
            } else {
                Err("Output not found".to_string())
            };
            let _ = reply.send(response);
        }

        BlockCommand::WriteInput(input_name, value, reply) => {
            let response = if let Some(input) = block.get_input_mut(&input_name) {
                let prev = input.get_value().cloned();
                input.set_value(value);
                Ok(prev)
            } else {
                Err("Input not found".to_string())
            };
            let _ = reply.send(response);
        }

        BlockCommand::GetInputWriter(input_name, reply) => {
            let response = if let Some(input) = block.get_input_mut(&input_name) {
                Ok(input.writer().clone())
            } else {
                Err(format!("Input '{}' not found", input_name))
            };
            let _ = reply.send(response);
        }

        BlockCommand::AddOutputLink {
            output_name,
            target_block_id,
            target_input_name,
            writer,
            reply,
        } => {
            let response = if let Some(output) = block.get_output_mut(&output_name) {
                if output.links().iter().any(|link| {
                    link.target_block_id() == &target_block_id
                        && link.target_input() == target_input_name
                }) {
                    Err("Already connected".to_string())
                } else {
                    use crate::base::link::{BaseLink, LinkState};

                    let mut link = BaseLink::new(target_block_id, target_input_name);
                    let id = link.id;

                    // Send current value before moving writer into link
                    if output.value().has_value() {
                        let _ = writer.try_send(output.value().clone());
                    }

                    link.tx = Some(writer);
                    link.state = LinkState::Connected;
                    output.add_link(link);
                    Ok(id)
                }
            } else if let Some(input) = block.get_input_mut(&output_name) {
                use crate::base::link::{BaseLink, LinkState};

                let mut link = BaseLink::new(target_block_id, target_input_name);
                let id = link.id;

                if let Some(val) = input.get_value().cloned() {
                    let _ = writer.try_send(val);
                }

                link.tx = Some(writer);
                link.state = LinkState::Connected;
                input.add_link(link);
                Ok(id)
            } else {
                Err(format!("Pin '{}' not found", output_name))
            };
            let _ = reply.send(response);
        }

        BlockCommand::AddInputLink {
            input_name,
            target_block_id,
            target_input_name,
            writer,
            reply,
        } => {
            let response = if let Some(input) = block.get_input_mut(&input_name) {
                use crate::base::link::{BaseLink, LinkState};

                let mut link = BaseLink::new(target_block_id, target_input_name);
                let id = link.id;
                link.tx = Some(writer);
                link.state = LinkState::Connected;
                input.add_link(link);
                Ok(id)
            } else {
                Err(format!("Input '{}' not found", input_name))
            };
            let _ = reply.send(response);
        }

        BlockCommand::RemoveLink(link_id, reply) => {
            let mut found = false;
            for output in block.outputs_mut() {
                if output.links().iter().any(|l| l.id() == &link_id) {
                    output.remove_link_by_id(&link_id);
                    found = true;
                    break;
                }
            }
            if !found {
                for input in block.inputs_mut() {
                    if input.links().iter().any(|l| l.id() == &link_id) {
                        input.remove_link_by_id(&link_id);
                        found = true;
                        break;
                    }
                }
            }
            let _ = reply.send(found);
        }

        BlockCommand::RemoveTargetBlockLinks(target_block_id) => {
            for output in block.outputs_mut() {
                output.remove_target_block_links(&target_block_id);
            }
            for input in block.inputs_mut() {
                input.remove_target_block_links(&target_block_id);
            }
        }

        BlockCommand::GetBlockData(reply) => {
            let block_data = BlockData {
                id: block.id().to_string(),
                name: block.name().to_string(),
                dis: block.desc().dis.to_string(),
                lib: block.desc().library.clone(),
                category: block.desc().category.clone(),
                ver: block.desc().ver.clone(),
            };

            let mut link_data = Vec::new();
            for (pin_name, pin_links) in block.links() {
                for link in pin_links {
                    link_data.push(LinkData {
                        id: Some(link.id().to_string()),
                        source_block_pin_name: pin_name.to_string(),
                        source_block_uuid: block.id().to_string(),
                        target_block_pin_name: link.target_input().to_string(),
                        target_block_uuid: link.target_block_id().to_string(),
                    });
                }
            }

            let _ = reply.send((block_data, link_data));
        }

        BlockCommand::SetState(state) => {
            block.set_state(state);
        }
    }
}

/// Send error replies for commands that expect responses when block is not found.
fn send_error_reply(cmd: BlockCommand) {
    match cmd {
        BlockCommand::Inspect(reply) => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::WriteOutput(_, _, reply) => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::WriteInput(_, _, reply) => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::GetInputWriter(_, reply) => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::AddOutputLink { reply, .. } => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::AddInputLink { reply, .. } => {
            let _ = reply.send(Err("Block not found".into()));
        }
        BlockCommand::RemoveLink(_, reply) => {
            let _ = reply.send(false);
        }
        BlockCommand::GetBlockData(_) => {}
        BlockCommand::RemoveTargetBlockLinks(_) => {}
        BlockCommand::SetState(_) => {}
    }
}

/// Async change-of-value check that works with Arc<RwLock<>> watchers.
async fn change_of_value_check<B: Block + 'static>(
    notification_channels: &Arc<RwLock<BTreeMap<Uuid, Sender<WatchMessage>>>>,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
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

    if !changes.is_empty() {
        for sender in notification_channels.read().await.values() {
            let _ = sender.try_send(WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
            });
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

        eng.schedule(add1);
        eng.schedule(sine1);
        eng.schedule(sine2);

        eng.run().await;
    }
}
