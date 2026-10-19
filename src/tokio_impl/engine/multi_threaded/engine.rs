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
//! Shares the per-block mailbox protocol (`block_mailbox`) and the
//! block graph (`graph`) with the
//! [`SingleThreadedEngine`](crate::single_threaded::SingleThreadedEngine),
//! so the actor task and every running-phase operation are the same —
//! the only MT-specific bits are watchers (`Arc<RwLock<...>>`) and the
//! cross-thread mailbox semantics, both of which already work over
//! [`tokio::spawn`].
//!
//! ## Runtime requirement
//!
//! The engine assumes it is running inside a tokio multi-thread runtime
//! (`#[tokio::main(flavor = "multi_thread")]` or a manually-constructed
//! `Runtime::new`). It does not create worker threads or runtimes of its
//! own — that's the caller's responsibility, and so is enabling tokio's
//! `rt-multi-thread`, which this crate never does (see the public
//! module docs).

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::runtime::Handle;

use crate::base::error::{EngineError, Result, parse_block_uuid};
use libhaystack::val::Value;
use tokio::sync::{
    RwLock,
    mpsc::{self, Receiver, Sender},
    watch,
};
use uuid::Uuid;

use super::super::block_mailbox::{BLOCK_MAILBOX_CAP, BlockMailboxCmd};
use super::super::connectors::{self, ManagedConnectors};
use super::super::graph::{BlockGraph, BlockHandle, ProgramHost, StagedProgram, stage_program};
use super::actor::{WatchersHandle, block_actor_task};
use crate::base::connector::{ConnectorHandle, register_connector};
use crate::base::{
    block::{Block, BlockDesc},
    engine::{
        Engine, EngineState, Idle, MaybeSendSync, Running,
        messages::{BlockDefinition, EngineMessage},
    },
    program::{LinkData, Position, Program},
};
use crate::blocks::registry::{BlockSink, get_block};
use crate::tokio_impl::engine::{
    Messages, ensure_schedulable_mt, phase_name, schedule_block_on_engine_mt,
};
use crate::tokio_impl::scan::ScanInterval;
use crate::tokio_impl::{MtBlock, ReaderImpl, WriterImpl};

/// Multi-threaded execution environment for blocks.
///
/// The type parameter is the engine's lifecycle phase — see the
/// [phase model](crate::base::engine#engine-phases). Configure an
/// [`Idle`] engine, then [`run`](Engine::run) it.
///
/// # Examples
///
/// `#[tokio::main]` builds a multi-thread runtime, so the application's
/// tokio needs its `rt-multi-thread` feature — see
/// [Runtime](crate::multi_threaded#runtime).
///
/// ```no_run
/// use logic_mesh::{
///     base::block::connect_output,
///     blocks::{math::Add, misc::SineWave},
///     prelude::*,
///     multi_threaded::MultiThreadedEngine,
/// };
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut sine = SineWave::new();
///     let mut add = Add::new();
///     connect_output(&mut sine.out, add.inputs_mut()[0])?;
///
///     let mut engine = MultiThreadedEngine::new();
///     engine.schedule(sine)?;
///     engine.schedule(add)?;
///     engine.run().await;
///     Ok(())
/// }
/// ```
pub struct MultiThreadedEngine<S: EngineState = Idle> {
    /// Every scheduled block's handle.
    graph: BlockGraph,
    /// Programs scheduled while idle, whose links and pin values are
    /// applied at `run()` start.
    pending: Vec<StagedProgram>,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
    pub(in super::super) reply_senders: BTreeMap<Uuid, Sender<Messages>>,
    pub(in super::super) watchers: WatchersHandle,
    /// Names of the connectors whose lifecycle this engine manages. The
    /// handles themselves live in the process-wide connector registry —
    /// the single source of truth blocks also resolve against. Dropping
    /// the engine unregisters them.
    connectors: ManagedConnectors,
    /// Execution gate every block actor subscribes to: `true` while the
    /// engine runs and is not paused. Actor tasks live on the ambient
    /// runtime from `schedule` on and keep serving their mailbox either
    /// way; the gate is what holds their blocks while idle or paused.
    execution: watch::Sender<bool>,
    /// The scan interval every block actor task runs with.
    scan: ScanInterval,
    /// The lifecycle phase, tracked in the type only.
    state: PhantomData<S>,
}

impl Default for MultiThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Shows the engine's phase, whether its blocks may execute, how many
/// blocks, links and staged programs it holds, the connectors it
/// manages and its scan interval — not the blocks or channels
/// themselves.
impl<S: EngineState> std::fmt::Debug for MultiThreadedEngine<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiThreadedEngine")
            .field("phase", &phase_name::<S>())
            .field("executing", &*self.execution.borrow())
            .field("blocks", &self.graph.block_count())
            .field("links", &self.graph.link_count())
            .field("pending_programs", &self.pending.len())
            .field("connectors", &self.connectors.as_slice())
            .field("scan_interval", &self.scan.get())
            .finish_non_exhaustive()
    }
}

impl Engine for MultiThreadedEngine<Idle> {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type Channel = Sender<Messages>;
    type RunFuture = BoxFuture<'static, Self>;

    /// Schedules a block on the engine. The [`Send`] + [`Sync`] bound
    /// lets the actor task go to [`tokio::spawn`], where the runtime is
    /// free to migrate it between worker threads.
    ///
    /// # Errors
    ///
    /// Besides the errors every engine reports, returns
    /// [`EngineError::NoRuntime`] when called outside a tokio runtime
    /// (`#[tokio::main]`, `Runtime::block_on`, or a task spawned on
    /// one): the block's actor task is spawned onto the ambient
    /// runtime as it is scheduled. The same holds for every other way
    /// of scheduling a block on this engine.
    fn schedule<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = Self::Writer, Reader = Self::Reader> + MaybeSendSync + 'static,
    {
        Ok(self.spawn_block(block)?)
    }

    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()> {
        let staged = stage_program(self, program)?;
        let (name, description) = staged.identity();
        self.graph.set_identity(name, description);
        self.pending.push(staged);
        Ok(())
    }

    fn add_connector(&mut self, name: &str, handle: impl Into<ConnectorHandle>) -> Result<()> {
        register_connector(name, handle)?;
        self.connectors.push(name.to_string());
        Ok(())
    }

    fn connector_names(&self) -> Vec<String> {
        self.connectors.to_vec()
    }

    fn block_handles(&self) -> Vec<&BlockHandle> {
        self.graph.handles()
    }

    fn block_handle(&self, id: &Uuid) -> Option<&BlockHandle> {
        self.graph.handle(id)
    }

    fn set_block_metadata(
        &mut self,
        id: &Uuid,
        label: Option<String>,
        position: Option<Position>,
    ) -> Result<()> {
        Ok(self.graph.set_metadata(id, label, position)?)
    }

    fn scan_interval(&self) -> Duration {
        self.scan.get()
    }

    fn set_scan_interval(&mut self, interval: Duration) {
        self.scan.set(interval);
    }

    fn run(self) -> Self::RunFuture {
        // The event loop evaluates blocks too (`EvaluateBlockReq`), so it
        // runs with the engine's scan interval like the actor tasks.
        let scan = self.scan.clone();
        Box::pin(scan.scope(async move {
            let mut engine = self.into_state::<Running>();
            engine.event_loop().await;
            engine.into_state()
        }))
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

impl MultiThreadedEngine<Idle> {
    /// Creates a new multi-threaded engine. The engine itself does not
    /// own worker threads — actor tasks are spawned onto whichever
    /// tokio multi-thread runtime the engine is running inside of.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);

        Self {
            graph: BlockGraph::default(),
            pending: Vec::new(),
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Arc::new(RwLock::new(BTreeMap::new())),
            connectors: ManagedConnectors::default(),
            execution: watch::Sender::new(false),
            scan: ScanInterval::default(),
            state: PhantomData,
        }
    }
}

impl<S: EngineState> MultiThreadedEngine<S> {
    /// Moves the engine to phase `T`, keeping all of its state.
    fn into_state<T: EngineState>(self) -> MultiThreadedEngine<T> {
        let Self {
            graph,
            pending,
            sender,
            receiver,
            reply_senders,
            watchers,
            connectors,
            execution,
            scan,
            state: _,
        } = self;
        MultiThreadedEngine {
            graph,
            pending,
            sender,
            receiver,
            reply_senders,
            watchers,
            connectors,
            execution,
            scan,
            state: PhantomData,
        }
    }

    /// Opens or closes the execution gate for every block.
    fn set_execution(&self, open: bool) {
        self.execution
            .send_if_modified(|current| std::mem::replace(current, open) != open);
    }

    /// Spawns `block`'s actor task onto the ambient runtime and records
    /// its handle. Blocks are added while configuring and, over the
    /// message channel, while running.
    ///
    /// Rejects an id that is already scheduled — see `spawn_block` on
    /// the single-threaded engine.
    fn spawn_block<B>(&mut self, block: B) -> Result<(), EngineError>
    where
        B: MtBlock + 'static,
    {
        // Checked before the block is recorded, so a failed schedule
        // leaves no handle without an actor behind.
        let runtime =
            Handle::try_current().map_err(|_| EngineError::NoRuntime { id: *block.id() })?;
        let (mailbox_tx, mailbox_rx) = mpsc::channel::<BlockMailboxCmd>(BLOCK_MAILBOX_CAP);
        self.graph.insert(&block, mailbox_tx)?;

        let watchers = self.watchers.clone();
        runtime.spawn(self.scan.scope(block_actor_task(
            block,
            mailbox_rx,
            watchers,
            self.execution.subscribe(),
        )));
        Ok(())
    }
}

impl<S: EngineState> BlockSink for MultiThreadedEngine<S> {
    fn accept<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = WriterImpl, Reader = ReaderImpl> + MaybeSendSync + 'static,
    {
        Ok(self.spawn_block(block)?)
    }
}

impl<S: EngineState> ProgramHost for MultiThreadedEngine<S> {
    fn graph(&self) -> &BlockGraph {
        &self.graph
    }

    fn graph_mut(&mut self) -> &mut BlockGraph {
        &mut self.graph
    }

    fn ensure_schedulable(&self, desc: &BlockDesc) -> Result<()> {
        ensure_schedulable_mt(desc)
    }

    fn schedule_desc(&mut self, desc: &BlockDesc, id: Uuid) -> Result<Uuid> {
        schedule_block_on_engine_mt(desc, Some(id), self)
    }
}

impl MultiThreadedEngine<Running> {
    /// The event loop: starts the managed connectors, wires the links
    /// and pushes the pin values of the programs scheduled while idle,
    /// opens the execution gate, then services engine messages until
    /// [`Shutdown`](EngineMessage::Shutdown) — see the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    async fn event_loop(&mut self) {
        // Start connectors first. MT actor tasks were spawned onto the
        // ambient runtime at `schedule` time, but the execution gate is
        // still closed, so no block executes against a connector that
        // has not started yet.
        connectors::start_connectors(&self.connectors).await;

        // Complete the programs scheduled during configuration. Actor
        // tasks are already live on the tokio MT runtime (they were
        // spawned at `schedule` time), so the mailbox round-trips
        // resolve immediately; with the gate closed, blocks first run
        // with their links and saved pin values in place.
        for staged in std::mem::take(&mut self.pending) {
            self.graph.complete_staged(staged).await;
        }

        // A run always starts unpaused.
        self.set_execution(true);
        loop {
            // The engine holds a sender itself, so the channel never
            // closes.
            let Some(message) = self.receiver.recv().await else {
                continue;
            };
            match message {
                EngineMessage::Shutdown => {
                    // Blocks stay scheduled, held by the closed gate,
                    // and resume on the next `run()`. Connectors stay
                    // registered so the re-run gets the same bindings.
                    //
                    // Stop-side ordering deviation from the ST engine:
                    // the gate closes before connectors stop, but an
                    // actor mid-`execute` on another worker only notices
                    // at its next await, so it can transiently hit a
                    // connector that is stopping here and fault. Benign:
                    // blocks fault-and-retry, and the engine is shutting
                    // down anyway.
                    self.set_execution(false);
                    connectors::stop_connectors(&self.connectors).await;
                    break;
                }
                EngineMessage::Reset => self.reset().await,
                EngineMessage::Pause => self.set_execution(false),
                EngineMessage::Resume => self.set_execution(true),
                message => self.dispatch_message(message).await,
            }
        }
    }

    /// Terminates and removes every block, then unregisters and stops
    /// the managed connectors.
    ///
    /// The actors are told to terminate without waiting on their
    /// mailboxes, as on the single-threaded engine. MT actors drain
    /// their mailboxes on the runtime, so an awaited send could not
    /// deadlock here, but it gains nothing either.
    async fn reset(&mut self) {
        self.graph.clear();
        // Unregister first — no block can `get_connector` a connector
        // that is being stopped — then stop the handles the
        // unregistration handed back. Terminated actor tasks are not
        // joined first, so a block mid-`execute` may transiently
        // resolve-and-fault against a stopping connector; blocks
        // fault-and-retry, so this is benign.
        connectors::unregister_and_stop_connectors(&mut self.connectors).await;
    }

    /// Adds a block to the engine by name and schedules it.
    fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<String>,
    ) -> Result<Uuid> {
        let block_def = get_block(block_name.as_str(), lib.as_deref())?;
        schedule_block_on_engine_mt(&block_def.desc, block_id, self)
    }

    /// Inspects the current state of a block.
    async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, EngineError> {
        self.graph.inspect_block(id).await
    }

    /// Writes a value to a block's input pin.
    async fn write_input(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Option<Value>, EngineError> {
        self.graph.write_input(id, name, value).await
    }

    /// Writes a value to a block's output pin.
    async fn write_output(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Value, EngineError> {
        self.graph.write_output(id, name, value).await
    }

    /// Connects two blocks (source pin → target input). The source pin can
    /// be either an output or an input (the latter is input-fanout).
    async fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData> {
        self.graph.connect_blocks(link_data).await
    }

    /// Removes a block and all its links from the engine.
    async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        self.graph.remove_block(block_id).await
    }

    /// Snapshots the full program: every scheduled block with its UI
    /// metadata and current pin values, plus every link. This is the
    /// canonical save format — round-trips through [`load_program`](Self::load_program).
    async fn save_program(&self) -> Result<Program> {
        self.graph.save_program().await
    }

    /// Loads a full [`Program`], all or nothing — see `load_program` on
    /// the [`SingleThreadedEngine`](crate::single_threaded::SingleThreadedEngine):
    /// the whole program is checked before anything is scheduled, and a
    /// failure while wiring removes the program's blocks again.
    async fn load_program(&mut self, program: Program) -> Result<()> {
        let staged = stage_program(self, &program)?;
        self.graph.load_staged(staged).await
    }

    /// Disconnects a link by its UUID.
    async fn disconnect_link_by_id(&mut self, link_id: &Uuid) -> Result<bool> {
        self.graph.disconnect_link_by_id(link_id).await
    }

    /// Sends `engine_message` to the reply channel registered under
    /// `sender_uuid`.
    ///
    /// Replies carry no correlation id — a caller matches replies to
    /// requests purely by order, which is why each channel must hold to
    /// the one-outstanding-request discipline documented on
    /// [`create_message_channel`](crate::base::engine::Engine::create_message_channel).
    /// `try_send` keeps the dispatcher from ever blocking on a slow
    /// caller: if the (capacity-32) channel is full, the reply is
    /// silently dropped.
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
                let block = match get_block(name.as_str(), lib.as_deref()) {
                    Ok(block) => block,
                    Err(err) => {
                        return self.reply_to_sender(
                            sender_uuid,
                            EngineMessage::EvaluateBlockRes(Err(err.to_string())),
                        );
                    }
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

            // Shared connector semantics with the single-threaded
            // engine — see the `connectors` module for the lifecycle
            // rules (start-failure cleanup, detach ordering, timeouts).
            EngineMessage::AddConnectorReq(sender_uuid, name) => {
                log::debug!("AddConnectorReq: {name}");

                let res = connectors::attach_connector(&mut self.connectors, name).await;
                self.reply_to_sender(sender_uuid, EngineMessage::AddConnectorRes(res));
            }

            EngineMessage::RemoveConnectorReq(sender_uuid, name) => {
                log::debug!("RemoveConnectorReq: {name}");

                let res = connectors::detach_connector(&mut self.connectors, &name).await;
                self.reply_to_sender(sender_uuid, EngineMessage::RemoveConnectorRes(res));
            }

            EngineMessage::ListConnectorsReq(sender_uuid) => {
                log::debug!("ListConnectorsReq");

                let res = Ok(self.connectors.to_vec());
                self.reply_to_sender(sender_uuid, EngineMessage::ListConnectorsRes(res));
            }

            // Lifecycle messages never reach dispatch, which leaves the
            // replies — see the single-threaded `dispatch_message`.
            other => log::warn!("Ignoring a message the engine does not handle: {other:?}"),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::base;
    use crate::base::program::LinkData;
    use crate::blocks::{math::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes, Shutdown};

    use super::MultiThreadedEngine;
    use base::engine::{Engine, Running};
    use tokio::sync::mpsc;
    use tokio::time::sleep;
    use uuid::Uuid;

    /// The engine and its block handles summarise themselves without
    /// dumping blocks, descriptors or channels.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn debug_summarises_the_engine_and_its_blocks() {
        let mut source = Add::new();
        let mut target = Add::new();
        let target_id = *target.id();
        source
            .connect_output("out", target.inputs_mut()[0])
            .expect("not linked yet");

        let mut eng = MultiThreadedEngine::new();
        eng.schedule(source).expect("scheduled");
        eng.schedule(target).expect("scheduled");

        assert_eq!(
            format!("{eng:?}"),
            "MultiThreadedEngine { phase: \"Idle\", executing: false, blocks: 2, links: 1, \
             pending_programs: 0, connectors: [], scan_interval: 200ms, .. }"
        );
        assert_eq!(
            format!("{:?}", eng.block_handle(&target_id).expect("scheduled")),
            format!(
                "BlockHandle {{ id: {target_id}, name: \"Add\", library: \"core\", \
                 label: None, position: None, .. }}"
            )
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn multi_threaded_engine_test() {
        use crate::base::block::connect_output;

        let mut add1 = Add::new();
        let add_uuid = *add1.id();

        let mut sine1 = SineWave::new();
        sine1.amplitude.set(3);
        sine1.freq.set(200);
        connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

        let mut sine2 = SineWave::new();
        sine2.amplitude.set(7);
        sine2.freq.set(400);
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

        eng.schedule(add1).unwrap();
        eng.schedule(sine1).unwrap();
        eng.schedule(sine2).unwrap();

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
    ///
    /// Drives the running-phase API directly, without the event loop:
    /// MT actor tasks live on the ambient runtime, so the mailbox
    /// round-trips resolve without `run()` — only the execution gate
    /// needs opening by hand.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn disconnect_link_severs_dataflow() {
        let mut eng = MultiThreadedEngine::new();

        let block_a = Add::new();
        let a_uuid = *block_a.id();
        let block_b = Add::new();
        let b_uuid = *block_b.id();

        eng.schedule(block_a).unwrap();
        eng.schedule(block_b).unwrap();
        let mut eng = eng.into_state::<Running>();
        eng.set_execution(true);

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

    /// Regression: a panic in `execute` killed the block's actor task —
    /// the block never showed Fault, downstream never faulted, and the
    /// block could no longer be saved or removed.
    mod panics {
        use super::*;
        use crate::tokio_impl::engine::test_support::panics;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn panic_faults_the_block_and_downstream() {
            panics::panic_faults_the_block_and_downstream(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn panicked_block_recovers_after_a_clean_cycle() {
            panics::panicked_block_recovers_after_a_clean_cycle(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_block_panicking_every_cycle_backs_off() {
            panics::a_block_panicking_every_cycle_backs_off(MultiThreadedEngine::new()).await;
        }
    }

    /// Actor tasks are detached on the ambient runtime, so they must end
    /// on their own once the engine is gone — see `test_support::drops`.
    mod drops {
        use super::*;
        use crate::tokio_impl::engine::test_support::drops;

        /// Regression: the dropped engine's closed mailboxes left every
        /// actor spinning on the runtime forever.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn dropping_an_idle_engine_ends_its_actors() {
            drops::dropping_an_idle_engine_ends_its_actors(MultiThreadedEngine::new()).await;
        }

        /// Shutdown keeps the actors now, so these too end only through
        /// their closed mailboxes.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn dropping_a_shut_down_engine_ends_its_actors() {
            drops::dropping_a_shut_down_engine_ends_its_actors(MultiThreadedEngine::new()).await;
        }

        /// Regression: as for the idle engine.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn dropping_the_run_future_ends_its_actors() {
            drops::dropping_the_run_future_ends_its_actors(MultiThreadedEngine::new()).await;
        }

        /// Regression: as for the idle engine, with `run()` on its own
        /// task, as an application spawns it.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn aborting_the_run_task_ends_its_actors() {
            let mut eng = MultiThreadedEngine::new();
            let alive = drops::schedule_tracked(&mut eng);

            let run = tokio::spawn(eng.run());
            sleep(Duration::from_millis(100)).await;
            run.abort();
            assert!(run.await.is_err_and(|err| err.is_cancelled()));

            drops::wait_for_actors_to_end(&alive).await;
        }
    }

    /// The lifecycle contract — see `test_support::lifecycle`.
    mod lifecycle {
        use super::*;
        use crate::tokio_impl::engine::test_support::lifecycle;

        /// Regression: shutdown terminated and dropped every block, so a
        /// re-run engine had none.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn blocks_survive_shutdown() {
            lifecycle::blocks_survive_shutdown(MultiThreadedEngine::new()).await;
        }

        /// Regression: a paused engine dropped requests without
        /// replying, and its blocks kept executing.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn pause_holds_blocks_and_answers_requests() {
            lifecycle::pause_holds_blocks_and_answers_requests(MultiThreadedEngine::new()).await;
        }

        /// Regression: requests sent after the reset went unanswered
        /// while paused.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn reset_while_paused() {
            lifecycle::reset_while_paused(MultiThreadedEngine::new()).await;
        }

        /// Regression: blocks executed from `schedule` on, before `run()`
        /// had started connectors and wired pending links.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn idle_engine_holds_blocks() {
            let mut eng = MultiThreadedEngine::new();
            let (source, target) =
                crate::tokio_impl::engine::test_support::schedule_linked_adds(&mut eng);
            let eng = eng.into_state::<Running>();

            eng.write_output(&source, "out".to_string(), 1.into())
                .await
                .expect("written");
            sleep(Duration::from_millis(200)).await;
            let target_out = eng.inspect_block(&target).await.expect("inspected").outputs["out"]
                .val
                .clone();
            assert_ne!(target_out, 1.into(), "no block executes while idle");
        }
    }

    /// Regression: see the single-threaded `pin_writes`.
    mod pin_writes {
        use super::*;
        use crate::tokio_impl::engine::test_support::pin_writes;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn writing_an_unlinked_block_recomputes_it() {
            pin_writes::writing_an_unlinked_block_recomputes_it(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn writing_a_constant_input_recomputes_a_linked_block() {
            pin_writes::writing_a_constant_input_recomputes_a_linked_block(
                MultiThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_fix_written_to_an_unlinked_input_clears_a_fault() {
            pin_writes::a_fix_written_to_an_unlinked_input_clears_a_fault(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: see the single-threaded `dead_actors`.
    mod dead_actors {
        use super::*;
        use crate::tokio_impl::engine::test_support::dead_actors;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_block_whose_actor_is_gone_is_saved_without_pins() {
            dead_actors::a_block_whose_actor_is_gone_is_saved_without_pins(
                MultiThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_link_out_of_a_gone_actor_is_removed() {
            dead_actors::a_link_out_of_a_gone_actor_is_removed(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_block_whose_actor_is_gone_is_removed() {
            dead_actors::a_block_whose_actor_is_gone_is_removed(MultiThreadedEngine::new()).await;
        }
    }

    /// Regression: see the single-threaded `runtime_descs`.
    mod runtime_descs {
        use super::*;
        use crate::tokio_impl::engine::test_support::runtime_descs;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_block_with_a_runtime_desc_runs_and_saves() {
            runtime_descs::a_block_with_a_runtime_desc_runs_and_saves(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_registered_block_loads_and_saves() {
            runtime_descs::a_registered_block_loads_and_saves(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_registered_block_is_found_by_its_name_alone() {
            runtime_descs::a_registered_block_is_found_by_its_name_alone(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: see the single-threaded `atomic_loads`.
    mod atomic_loads {
        use super::*;
        use crate::tokio_impl::engine::test_support::atomic_loads;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_failing_its_checks_loads_nothing() {
            atomic_loads::a_program_failing_its_checks_loads_nothing(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_failing_its_checks_schedules_nothing() {
            atomic_loads::a_program_failing_its_checks_schedules_nothing(
                MultiThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_failing_to_wire_is_removed_again() {
            atomic_loads::a_program_failing_to_wire_is_removed_again(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_scheduled_while_idle_drops_a_link_it_fails_to_wire() {
            atomic_loads::a_program_scheduled_while_idle_drops_a_link_it_fails_to_wire(
                MultiThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_scheduled_while_idle_gets_its_pin_values() {
            atomic_loads::a_program_scheduled_while_idle_gets_its_pin_values(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: see the single-threaded `round_trip`.
    mod round_trip {
        use super::*;
        use crate::tokio_impl::engine::test_support::round_trip;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_saved_program_reloads_unchanged() {
            round_trip::a_saved_program_reloads_unchanged(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn reset_forgets_the_program_identity() {
            round_trip::reset_forgets_the_program_identity(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_scheduled_while_idle_saves_its_identity() {
            round_trip::a_program_scheduled_while_idle_saves_its_identity(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: see the single-threaded `scan_intervals`.
    mod scan_intervals {
        use super::*;
        use crate::tokio_impl::engine::test_support::scan_intervals;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn each_engine_keeps_its_own_scan_interval() {
            scan_intervals::each_engine_keeps_its_own_scan_interval(
                MultiThreadedEngine::new(),
                MultiThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_new_scan_interval_reaches_scheduled_blocks() {
            scan_intervals::a_new_scan_interval_reaches_scheduled_blocks(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: see the single-threaded `configuration`.
    mod configuration {
        use super::*;
        use crate::tokio_impl::engine::test_support::configuration;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn an_engine_is_configured_through_the_trait() {
            configuration::an_engine_is_configured_through_the_trait(MultiThreadedEngine::new())
                .await;
        }
    }

    /// Regression: scheduling outside a tokio runtime panicked inside
    /// `tokio::spawn`, although `schedule` returns a `Result`.
    #[test]
    fn scheduling_outside_a_runtime_is_an_error() {
        use crate::blocks::registry::{CORE_LIB, schedule_block};
        use crate::tokio_impl::engine::test_support::add_program;
        use crate::{EngineError, Error};
        use assert_matches::assert_matches;

        let mut eng = MultiThreadedEngine::new();
        let add = Add::new();
        let id = *add.id();
        assert_matches!(
            eng.schedule(add),
            Err(Error::Engine(EngineError::NoRuntime { id: refused })) if refused == id
        );
        assert_matches!(
            schedule_block("Add", Some(CORE_LIB), &mut eng),
            Err(Error::Engine(EngineError::NoRuntime { .. }))
        );
        let program = add_program([Uuid::new_v4().to_string().as_str()]);
        assert_matches!(
            eng.schedule_program_blocks(&program),
            Err(Error::Engine(EngineError::NoRuntime { .. }))
        );
        assert!(eng.block_handles().is_empty());
    }

    /// The multi-threaded engine's run future is `Send`, and code
    /// generic over [`Engine`] can require that and spawn it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn generic_code_can_spawn_the_run_future() {
        fn spawn_run<E>(engine: E) -> tokio::task::JoinHandle<E>
        where
            E: Engine + Send + 'static,
            E::RunFuture: Send + 'static,
        {
            tokio::spawn(engine.run())
        }

        let mut eng = MultiThreadedEngine::new();
        let (reply, _replies) = mpsc::channel(1);
        let sender = eng.create_message_channel(Uuid::new_v4(), reply);
        let running = spawn_run(eng);
        sender.send(Shutdown).await.expect("engine listening");
        tokio::time::timeout(Duration::from_secs(5), running)
            .await
            .expect("engine shut down")
            .expect("run task completed");
    }

    /// Regression: see the single-threaded `rejections`.
    mod rejections {
        use super::*;
        use crate::tokio_impl::engine::test_support::rejections;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn writing_a_missing_pin_names_it() {
            rejections::writing_a_missing_pin_names_it(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn linking_the_same_pins_twice_is_a_connect_error() {
            rejections::linking_the_same_pins_twice_is_a_connect_error(MultiThreadedEngine::new())
                .await;
        }
    }

    /// Regression: see the single-threaded `link_ids`.
    mod link_ids {
        use super::*;
        use crate::tokio_impl::engine::test_support::link_ids;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn connecting_keeps_a_named_link_id() {
            link_ids::connecting_keeps_a_named_link_id(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn connecting_rejects_a_taken_or_invalid_link_id() {
            link_ids::connecting_rejects_a_taken_or_invalid_link_id(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_loaded_link_keeps_its_saved_id() {
            link_ids::a_loaded_link_keeps_its_saved_id(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn a_program_reusing_a_link_id_is_rejected() {
            link_ids::a_program_reusing_a_link_id_is_rejected(MultiThreadedEngine::new()).await;
        }
    }

    /// Regression: see the single-threaded `connector_deadlines`.
    mod connector_deadlines {
        use super::*;
        use crate::tokio_impl::engine::test_support::connector_deadlines;

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn external_out_times_out_under_requests() {
            connector_deadlines::external_out_times_out_under_requests(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn request_times_out_under_requests() {
            connector_deadlines::request_times_out_under_requests(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn external_in_times_out_under_requests() {
            connector_deadlines::external_in_times_out_under_requests(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn external_out_applies_a_late_timeout() {
            connector_deadlines::external_out_applies_a_late_timeout(MultiThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn request_applies_a_late_timeout() {
            connector_deadlines::request_applies_a_late_timeout(MultiThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn external_in_applies_a_late_timeout() {
            connector_deadlines::external_in_applies_a_late_timeout(MultiThreadedEngine::new())
                .await;
        }

        /// Regression: on this engine a loaded block runs on another
        /// worker while the load writes its saved values, so it issued
        /// its operation between the `in` and `timeout` writes, under
        /// the default deadline, and kept that deadline.
        #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
        async fn a_loaded_timeout_applies_to_the_first_operation() {
            connector_deadlines::a_loaded_timeout_applies_to_the_first_operation(
                MultiThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: a reply variant sent to the engine hit an
    /// `unreachable!` and panicked the event loop. It is now ignored,
    /// and the engine keeps answering.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stray_reply_is_ignored() {
        use crate::base::engine::messages::EngineMessage::{
            AddBlockRes, ListConnectorsReq, ListConnectorsRes,
        };
        use crate::tokio_impl::engine::test_support::drive;

        drive(MultiThreadedEngine::new(), |mut client| async move {
            client.send(AddBlockRes(Ok(Uuid::new_v4()))).await;

            let reply = client.request(ListConnectorsReq(client.id)).await;
            assert!(
                matches!(reply, ListConnectorsRes(Ok(_))),
                "Expected ListConnectorsRes(Ok), got {:?}",
                reply
            );
        })
        .await;
    }

    /// Regression: scheduling an id that is already taken used to
    /// replace the scheduled block's handle, orphaning its actor.
    mod duplicate_ids {
        use assert_matches::assert_matches;

        use super::*;
        use crate::base::engine::messages::EngineMessage::{
            AddBlockReq, AddBlockRes, GetCurrentProgramReq, GetCurrentProgramRes, LoadProgramReq,
            LoadProgramRes,
        };
        use crate::base::error::{EngineError, Error};
        use crate::tokio_impl::engine::test_support::{Client, add_program, drive};

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn schedule_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let mut eng = MultiThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("first schedule");

            let err = eng
                .schedule(Add::new_uuid(id))
                .expect_err("second schedule is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert_eq!(eng.block_handles().len(), 1);
        }

        /// The whole program is checked up front: the program's other,
        /// fresh block is not scheduled either.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn schedule_program_blocks_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let fresh = Uuid::new_v4();
            let mut eng = MultiThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            let program = add_program([id.to_string().as_str(), fresh.to_string().as_str()]);
            let err = eng
                .schedule_program_blocks(&program)
                .expect_err("program reusing a scheduled id is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert!(eng.block_handle(&fresh).is_none(), "nothing was scheduled");
            assert_eq!(eng.block_handles().len(), 1);
        }

        /// Two program keys can spell the same UUID differently.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn schedule_program_blocks_rejects_an_id_named_twice() {
            let id = Uuid::new_v4();
            let mut eng = MultiThreadedEngine::new();

            let lower = id.to_string();
            let upper = lower.to_uppercase();
            let err = eng
                .schedule_program_blocks(&add_program([lower.as_str(), upper.as_str()]))
                .expect_err("program naming an id twice is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert!(eng.block_handles().is_empty(), "nothing was scheduled");
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn add_block_req_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let mut eng = MultiThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            drive(eng, move |mut client| async move {
                let reply = client
                    .request(AddBlockReq(
                        client.id,
                        "Add".to_string(),
                        Some(id.to_string()),
                        None,
                    ))
                    .await;
                assert_matches!(
                    reply,
                    AddBlockRes(Err(msg))
                        if msg == EngineError::DuplicateBlockId { id }.to_string()
                );

                // The scheduled block is untouched and still answers.
                let reply = client.request(InspectBlockReq(client.id, id)).await;
                assert_matches!(reply, InspectBlockRes(Ok(def)) if def.id == id.to_string());
                assert_eq!(scheduled_ids(&mut client).await, vec![id.to_string()]);
            })
            .await;
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn load_program_req_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let fresh = Uuid::new_v4();
            let mut eng = MultiThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            let program = add_program([id.to_string().as_str(), fresh.to_string().as_str()]);
            drive(eng, move |mut client| async move {
                let reply = client.request(LoadProgramReq(client.id, program)).await;
                assert_matches!(
                    reply,
                    LoadProgramRes(Err(msg))
                        if msg == EngineError::DuplicateBlockId { id }.to_string()
                );
                assert_eq!(
                    scheduled_ids(&mut client).await,
                    vec![id.to_string()],
                    "nothing was scheduled"
                );
            })
            .await;
        }

        /// The ids of the running engine's blocks, from its saved program.
        async fn scheduled_ids(client: &mut Client) -> Vec<String> {
            match client.request(GetCurrentProgramReq(client.id)).await {
                GetCurrentProgramRes(Ok(program)) => program.blocks.into_keys().collect(),
                other => panic!("Expected GetCurrentProgramRes(Ok), got {:?}", other),
            }
        }
    }

    /// Connector lifecycle on the MT engine
    mod connector_lifecycle {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };

        use super::*;
        use crate::base::connector::{get_connector, register_connector, unregister_connector};
        use crate::base::engine::messages::EngineMessage::{
            AddConnectorReq, AddConnectorRes, ListConnectorsReq, ListConnectorsRes,
            RemoveConnectorReq, RemoveConnectorRes, Reset,
        };
        use crate::tokio_impl::engine::connectors::test_support::{
            FailingStartConnector, FlagConnector, start_failure_reply,
        };

        /// The connector registry is process-global and tests run in
        /// parallel, so every test uses a unique name.
        fn setup(
            prefix: &str,
        ) -> (
            String,
            Arc<AtomicBool>,
            Arc<AtomicBool>,
            MultiThreadedEngine,
        ) {
            let name = format!("{prefix}-{}", Uuid::new_v4());
            let started = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));

            let mut eng = MultiThreadedEngine::new();
            eng.add_connector(
                &name,
                Arc::new(FlagConnector {
                    started: started.clone(),
                    stopped: stopped.clone(),
                }),
            )
            .expect("connector added");

            (name, started, stopped, eng)
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn duplicate_name_is_rejected() {
            let (name, _, _, mut eng) = setup("mt-dup");
            let dup = Arc::new(FlagConnector {
                started: Arc::default(),
                stopped: Arc::default(),
            });
            eng.add_connector(&name, dup)
                .expect_err("duplicate connector name is rejected");
            assert_eq!(
                eng.connector_names().iter().filter(|n| *n == &name).count(),
                1,
                "rejected add does not double-track the name"
            );
            assert!(
                get_connector(&name).is_some(),
                "rejected add leaves the original handle registered"
            );
            unregister_connector(&name);
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn started_on_run_and_stopped_on_shutdown() {
            let (name, started, stopped, mut eng) = setup("mt-shutdown");

            let (sender, _receiver) = mpsc::channel(32);
            let engine_sender = eng.create_message_channel(Uuid::new_v4(), sender);

            // On the MT runtime the driver runs concurrently with
            // `run()` as a plain task — no extra thread/runtime needed.
            let driver = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;
                let _ = engine_sender.send(Shutdown).await;
            });

            let eng = eng.run().await;
            driver.await.unwrap();

            assert!(started.load(Ordering::SeqCst), "start driven by run()");
            assert!(stopped.load(Ordering::SeqCst), "stop driven on shutdown");
            assert!(
                get_connector(&name).is_some(),
                "shutdown keeps the connector registered for a re-run"
            );
            drop(eng);
            assert!(
                get_connector(&name).is_none(),
                "dropping the engine unregisters the connector"
            );
        }

        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn reset_stops_and_unregisters() {
            let (name, started, stopped, mut eng) = setup("mt-reset");

            let (sender, _receiver) = mpsc::channel(32);
            let engine_sender = eng.create_message_channel(Uuid::new_v4(), sender);

            let driver = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;
                let _ = engine_sender.send(Reset).await;
                let _ = engine_sender.send(Shutdown).await;
            });

            eng.run().await;
            driver.await.unwrap();

            assert!(started.load(Ordering::SeqCst), "start driven by run()");
            assert!(stopped.load(Ordering::SeqCst), "stop driven on reset");
            assert!(
                get_connector(&name).is_none(),
                "reset unregisters the connector"
            );
        }

        /// A connector registered globally while the engine is running
        /// can be attached, listed, and detached over the message
        /// channel.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn dynamic_add_list_remove_over_messages() {
            let name = format!("mt-dynamic-{}", Uuid::new_v4());
            let started = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));

            // Registered globally, NOT engine-managed yet — attaching
            // over the message channel is what puts it under
            // engine management.
            register_connector(
                &name,
                Arc::new(FlagConnector {
                    started: started.clone(),
                    stopped: stopped.clone(),
                }),
            )
            .expect("registered");

            let mut eng = MultiThreadedEngine::new();
            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver_started = started.clone();
            let driver_stopped = stopped.clone();
            let driver = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;

                let _ = engine_sender
                    .send(AddConnectorReq(channel_id, driver_name.clone()))
                    .await;
                match receiver.recv().await {
                    Some(AddConnectorRes(Ok(added))) => assert_eq!(added, driver_name),
                    other => panic!("Expected AddConnectorRes(Ok), got {:?}", other),
                }
                assert!(
                    driver_started.load(Ordering::SeqCst),
                    "attach awaits the connector's start"
                );

                let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                match receiver.recv().await {
                    Some(ListConnectorsRes(Ok(names))) => {
                        assert!(names.contains(&driver_name), "attached name is listed")
                    }
                    other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                }

                let _ = engine_sender
                    .send(RemoveConnectorReq(channel_id, driver_name.clone()))
                    .await;
                match receiver.recv().await {
                    Some(RemoveConnectorRes(Ok(removed))) => assert_eq!(removed, driver_name),
                    other => panic!("Expected RemoveConnectorRes(Ok), got {:?}", other),
                }
                assert!(
                    driver_stopped.load(Ordering::SeqCst),
                    "detach awaits the connector's stop"
                );
                assert!(
                    get_connector(&driver_name).is_none(),
                    "detach unregisters the connector"
                );

                let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                match receiver.recv().await {
                    Some(ListConnectorsRes(Ok(names))) => {
                        assert!(!names.contains(&driver_name), "detached name is not listed")
                    }
                    other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                }

                let _ = engine_sender.send(Shutdown).await;
            });

            eng.run().await;
            driver.await.unwrap();
        }

        /// A failed attach must not leak a partially-started connector:
        /// `start` may have spawned IO tasks before erroring, so the
        /// engine stops the handle before reporting the error. The
        /// name stays registered so a retry remains possible.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn failed_attach_stops_the_connector() {
            let name = format!("mt-failing-start-{}", Uuid::new_v4());
            let stopped = Arc::new(AtomicBool::new(false));
            register_connector(
                &name,
                Arc::new(FailingStartConnector {
                    stopped: stopped.clone(),
                }),
            )
            .expect("registered");

            let mut eng = MultiThreadedEngine::new();
            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver_stopped = stopped.clone();
            let driver = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;

                let _ = engine_sender
                    .send(AddConnectorReq(channel_id, driver_name.clone()))
                    .await;
                let reply = match receiver.recv().await {
                    Some(AddConnectorRes(Err(reply))) => reply,
                    other => panic!(
                        "Expected AddConnectorRes(Err) on start failure, got {:?}",
                        other
                    ),
                };
                assert!(
                    driver_stopped.load(Ordering::SeqCst),
                    "failed attach stops the connector before replying"
                );
                assert!(
                    get_connector(&driver_name).is_some(),
                    "failed attach keeps the connector registered for a retry"
                );

                let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                match receiver.recv().await {
                    Some(ListConnectorsRes(Ok(names))) => {
                        assert!(
                            !names.contains(&driver_name),
                            "failed attach is not tracked"
                        )
                    }
                    other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                }

                let _ = engine_sender.send(Shutdown).await;
                reply
            });

            eng.run().await;
            let reply = driver.await.unwrap();
            unregister_connector(&name);

            // The reply carries the start error's source too
            // (regression: it printed only the error's own message).
            assert_eq!(reply, start_failure_reply(&name));
        }

        /// Attach/detach error cases: an unregistered name, a name the
        /// engine already manages, and detaching an unmanaged name.
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn dynamic_add_remove_error_cases() {
            let (name, _, _, mut eng) = setup("mt-dynamic-errors");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver = tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;

                let missing = format!("mt-missing-{}", Uuid::new_v4());
                let _ = engine_sender
                    .send(AddConnectorReq(channel_id, missing.clone()))
                    .await;
                match receiver.recv().await {
                    Some(AddConnectorRes(Err(_))) => {}
                    other => panic!(
                        "Expected AddConnectorRes(Err) for an unregistered name, got {:?}",
                        other
                    ),
                }

                let _ = engine_sender
                    .send(AddConnectorReq(channel_id, driver_name.clone()))
                    .await;
                match receiver.recv().await {
                    Some(AddConnectorRes(Err(_))) => {}
                    other => panic!(
                        "Expected AddConnectorRes(Err) for an already-managed name, got {:?}",
                        other
                    ),
                }

                let _ = engine_sender
                    .send(RemoveConnectorReq(channel_id, missing))
                    .await;
                match receiver.recv().await {
                    Some(RemoveConnectorRes(Err(_))) => {}
                    other => panic!(
                        "Expected RemoveConnectorRes(Err) for an unmanaged name, got {:?}",
                        other
                    ),
                }

                let _ = engine_sender.send(Shutdown).await;
            });

            eng.run().await;
            driver.await.unwrap();

            unregister_connector(&name);
        }
    }
}
