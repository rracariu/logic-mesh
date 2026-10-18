// Copyright (c) 2022-2026, Radu Racariu.

//! [`SingleThreadedEngine`] — engine struct, lifecycle, and the
//! synchronous + async API surface that messaging dispatch routes to.

use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, rc::Rc};

use crate::base::error::{EngineError, RegistryError, Result};
use libhaystack::val::Value;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender, UnboundedSender},
        watch,
    },
    task::LocalSet,
};
use uuid::Uuid;

use super::super::block_mailbox::{BLOCK_MAILBOX_CAP, BlockMailboxCmd};
use super::super::connectors::{self, ManagedConnectors};
#[cfg(test)]
use super::super::graph::LinkRecord;
use super::super::graph::{BlockGraph, BlockHandle, ProgramHost, StagedProgram, stage_program};
use super::actor::block_actor_task;
use crate::base::connector::{ConnectorHandle, register_connector};
use crate::base::{
    block::{Block, BlockDesc},
    engine::{
        Engine, EngineState, Idle, MaybeSendSync, Running,
        messages::{BlockDefinition, EngineMessage, WatchMessage},
    },
    program::{
        Program,
        data::{LinkData, Position},
    },
};
use crate::blocks::registry::{BlockSink, CORE_LIB, get_block};
use crate::tokio_impl::engine::Messages;
use crate::tokio_impl::engine::message_dispatch::dispatch_message;
use crate::tokio_impl::engine::{ensure_schedulable, phase_name, schedule_block_on_engine};
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// Single-threaded execution environment for blocks.
///
/// The type parameter is the engine's lifecycle phase — see the
/// [phase model](crate::base::engine#engine-phases). Configure an
/// [`Idle`] engine, then [`run`](Engine::run) it.
///
/// # Examples
///
/// ```no_run
/// use logic_mesh::{
///     base::block::{Block, BlockProps, connect_output},
///     base::engine::Engine,
///     blocks::{math::Add, misc::SineWave},
///     single_threaded::SingleThreadedEngine,
/// };
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut sine = SineWave::new();
///     let mut add = Add::new();
///     connect_output(&mut sine.out, add.inputs_mut()[0])?;
///
///     let mut engine = SingleThreadedEngine::new();
///     engine.schedule(sine)?;
///     engine.schedule(add)?;
///     engine.run().await;
///     Ok(())
/// }
/// ```
pub struct SingleThreadedEngine<S: EngineState = Idle> {
    /// [`LocalSet`](tokio::task::LocalSet) that hosts every per-block actor
    /// task. Wrapped in [`Rc`](std::rc::Rc) so
    /// `run()` can clone a handle and avoid double-borrowing `self` when it
    /// needs to drive the local set while also dispatching engine messages.
    local: Rc<LocalSet>,
    /// Every scheduled block's handle. Populated by `schedule()` (each
    /// block spawns its actor task immediately).
    graph: BlockGraph,
    /// Programs scheduled while idle, whose links and pin values are
    /// applied at `run()` start (which pumps the LocalSet so the actor
    /// tasks can answer the round-trips).
    pending: Vec<StagedProgram>,
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
    /// Names of the connectors whose lifecycle this engine manages. The
    /// handles themselves live in the process-wide connector registry —
    /// the single source of truth blocks also resolve against. Dropping
    /// the engine unregisters them.
    connectors: ManagedConnectors,
    /// Execution gate every block actor subscribes to: `true` while the
    /// engine runs and is not paused. Actors keep serving their mailbox
    /// either way, so requests are answered while blocks are held.
    execution: watch::Sender<bool>,
    /// The lifecycle phase, tracked in the type only.
    state: PhantomData<S>,
}

impl Default for SingleThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Shows the engine's phase, whether its blocks may execute, how many
/// blocks, links and staged programs it holds and the connectors it
/// manages — not the blocks or channels themselves.
impl<S: EngineState> std::fmt::Debug for SingleThreadedEngine<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleThreadedEngine")
            .field("phase", &phase_name::<S>())
            .field("executing", &*self.execution.borrow())
            .field("blocks", &self.graph.block_count())
            .field("links", &self.graph.link_count())
            .field("pending_programs", &self.pending.len())
            .field("connectors", &self.connectors.as_slice())
            .finish_non_exhaustive()
    }
}

impl Engine for SingleThreadedEngine<Idle> {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type Channel = Sender<Messages>;

    fn schedule<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = Self::Writer, Reader = Self::Reader> + MaybeSendSync + 'static,
    {
        Ok(self.spawn_block(block)?)
    }

    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()> {
        // Wiring and pin writes are async (mailbox round-trips); queue
        // the checked program for completion at `run()` start.
        let staged = stage_program(self, program)?;
        let (name, description) = staged.identity();
        self.graph.set_identity(name, description);
        self.pending.push(staged);
        Ok(())
    }

    async fn run(self) -> Self {
        let mut engine = self.into_state::<Running>();
        engine.event_loop().await;
        engine.into_state()
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

impl SingleThreadedEngine<Idle> {
    /// Creates a new single-threaded engine.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(32);
        Self {
            local: Rc::new(LocalSet::new()),
            graph: BlockGraph::default(),
            pending: Vec::new(),
            sender,
            receiver,
            reply_senders: BTreeMap::new(),
            watchers: Rc::default(),
            connectors: ManagedConnectors::default(),
            execution: watch::Sender::new(false),
            state: PhantomData,
        }
    }

    /// Schedules a block that need not be [`Send`] or [`Sync`].
    ///
    /// Blocks run on the engine's [`LocalSet`](tokio::task::LocalSet), so
    /// unlike [`Engine::schedule`] — whose bound lets every engine host
    /// the block — this accepts blocks that are not thread-safe.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    /// use logic_mesh::{
    ///     BlockProps, block,
    ///     base::block::Block,
    ///     blocks::{InputImpl, OutputImpl},
    ///     single_threaded::SingleThreadedEngine,
    /// };
    ///
    /// #[block]
    /// #[derive(BlockProps, Debug)]
    /// #[category = "example"]
    /// struct Counter {
    ///     #[input(kind = "Number")]
    ///     input: InputImpl,
    ///     #[output(kind = "Number")]
    ///     out: OutputImpl,
    ///     count: Cell<u64>,
    /// }
    ///
    /// impl Block for Counter {
    ///     async fn execute(&mut self) {
    ///         self.count.set(self.count.get() + 1);
    ///     }
    /// }
    ///
    /// let mut engine = SingleThreadedEngine::new();
    /// engine.schedule_local(Counter::new())?;
    /// assert_eq!(engine.block_handles().len(), 1);
    /// # Ok::<(), logic_mesh::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DuplicateBlockId`] if a block with the same
    /// id is already scheduled.
    pub fn schedule_local<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
    {
        Ok(self.spawn_block(block)?)
    }

    /// Registers `handle` in the process-wide connector registry under
    /// `name` and puts it under this engine's lifecycle management.
    ///
    /// The engine awaits the connector's `start` when [`run`](Engine::run)
    /// begins — before any block actor makes progress, so blocks never
    /// resolve an unstarted connector — and its `stop` on shutdown; the
    /// connector stays registered so a re-run picks it up again. A reset
    /// unregisters the connector first and then stops it. Dropping the
    /// engine unregisters any still-tracked connectors without awaiting
    /// `stop`. Start/stop failures and timeouts are logged, not fatal.
    ///
    /// Connectors own their IO loops: spawn them from `start` (via the
    /// ambient `tokio::spawn` natively, `wasm_bindgen_futures::spawn_local`
    /// on wasm) and wind them down in `stop`. Pausing the engine does not
    /// pause these tasks — pause only stops block execution.
    ///
    /// # Errors
    ///
    /// Returns an error if a connector with this name is already
    /// registered.
    pub fn add_connector(&mut self, name: &str, handle: impl Into<ConnectorHandle>) -> Result<()> {
        register_connector(name, handle)?;
        self.connectors.push(name.to_string());
        Ok(())
    }
}

impl<S: EngineState> SingleThreadedEngine<S> {
    /// Moves the engine to phase `T`, keeping all of its state.
    fn into_state<T: EngineState>(self) -> SingleThreadedEngine<T> {
        let Self {
            local,
            graph,
            pending,
            sender,
            receiver,
            reply_senders,
            watchers,
            connectors,
            execution,
            state: _,
        } = self;
        SingleThreadedEngine {
            local,
            graph,
            pending,
            sender,
            receiver,
            reply_senders,
            watchers,
            connectors,
            execution,
            state: PhantomData,
        }
    }

    /// Opens or closes the execution gate for every block.
    fn set_execution(&self, open: bool) {
        self.execution
            .send_if_modified(|current| std::mem::replace(current, open) != open);
    }

    /// The names of the connectors this engine manages.
    pub fn connector_names(&self) -> Vec<String> {
        self.connectors.to_vec()
    }

    /// Returns sync metadata handles for every scheduled block. Use the
    /// async snapshot APIs (`inspect_block`, etc.) to read dynamic state.
    pub fn block_handles(&self) -> Vec<&BlockHandle> {
        self.graph.handles()
    }

    /// Returns the handle for a specific block, if scheduled.
    pub fn block_handle(&self, id: &Uuid) -> Option<&BlockHandle> {
        self.graph.handle(id)
    }

    // --- Sync engine ops (any phase) ---------------------------------------

    /// Spawns `block`'s actor task and records its handle. Blocks are
    /// added while configuring and, over the message channel, while
    /// running.
    ///
    /// Rejects an id that is already scheduled: replacing the handle
    /// would orphan the running block, whose links other blocks still
    /// point at.
    fn spawn_block<B>(&mut self, block: B) -> Result<(), EngineError>
    where
        B: Block<Writer = WriterImpl, Reader = ReaderImpl> + 'static,
    {
        let (mailbox_tx, mailbox_rx) = mpsc::channel::<BlockMailboxCmd>(BLOCK_MAILBOX_CAP);
        self.graph.insert(&block, mailbox_tx)?;

        let watchers = self.watchers.clone();
        self.local.spawn_local(block_actor_task(
            block,
            mailbox_rx,
            watchers,
            self.execution.subscribe(),
        ));
        Ok(())
    }

    /// Validates `link_data` against the scheduled blocks — see
    /// `BlockGraph::validate_link`.
    #[cfg(test)]
    pub(super) fn validate_link(&self, link_data: &LinkData) -> Result<(Uuid, LinkRecord)> {
        self.graph.validate_link(link_data)
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
        self.graph.set_metadata(id, label, position);
    }
}

impl<S: EngineState> BlockSink for SingleThreadedEngine<S> {
    fn accept<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = WriterImpl, Reader = ReaderImpl> + MaybeSendSync + 'static,
    {
        Ok(self.spawn_block(block)?)
    }
}

impl<S: EngineState> ProgramHost for SingleThreadedEngine<S> {
    fn graph(&self) -> &BlockGraph {
        &self.graph
    }

    fn graph_mut(&mut self) -> &mut BlockGraph {
        &mut self.graph
    }

    fn ensure_schedulable(&self, desc: &BlockDesc) -> Result<()> {
        ensure_schedulable(desc)
    }

    fn schedule_desc(&mut self, desc: &BlockDesc, id: Uuid) -> Result<Uuid> {
        schedule_block_on_engine(desc, Some(id), self)
    }
}

impl SingleThreadedEngine<Running> {
    /// The event loop: starts the managed connectors, wires the links
    /// and pushes the pin values of the programs scheduled while idle,
    /// opens the execution gate, then services engine messages until
    /// [`Shutdown`](EngineMessage::Shutdown) — see the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    async fn event_loop(&mut self) {
        // Clone the LocalSet handle so the loop body can drive `local`
        // and mutate `self` independently of each other.
        let local = self.local.clone();

        // Start connectors before anything drives the LocalSet: block
        // actors are already spawned at schedule time, so any
        // `run_until` would let them run against a connector that has
        // not started yet. A plain await makes no progress on the
        // LocalSet, so every connector is started before any actor runs.
        connectors::start_connectors(&self.connectors).await;

        // Complete the programs scheduled during configuration. The gate
        // is still closed, so actors answer the round-trips without
        // executing: blocks first run with their links and saved pin
        // values in place.
        for staged in std::mem::take(&mut self.pending) {
            local.run_until(self.graph.complete_staged(staged)).await;
        }

        // A run always starts unpaused.
        self.set_execution(true);
        loop {
            // The LocalSet is driven while waiting in every state: with
            // the gate closed, actors serve their mailboxes but no block
            // executes.
            let mut engine_msg = None;
            local
                .run_until(async {
                    engine_msg = self.receiver.recv().await;
                })
                .await;

            // The engine holds a sender itself, so the channel never
            // closes.
            let Some(message) = engine_msg else {
                continue;
            };
            match message {
                EngineMessage::Shutdown => {
                    // Blocks stay scheduled, held by the closed gate,
                    // and resume on the next `run()`. Connectors stay
                    // registered so the re-run gets the same bindings.
                    self.set_execution(false);
                    connectors::stop_connectors(&self.connectors).await;
                    break;
                }
                EngineMessage::Reset => self.reset().await,
                EngineMessage::Pause => self.set_execution(false),
                EngineMessage::Resume => self.set_execution(true),
                message => local.run_until(dispatch_message(self, message)).await,
            }
        }
    }

    /// Terminates and removes every block, then unregisters and stops
    /// the managed connectors.
    ///
    /// Runs outside `run_until`, where no actor makes progress, so the
    /// actors are told to terminate without waiting on their mailboxes.
    async fn reset(&mut self) {
        self.graph.clear();
        // Unregister first — no block can `get_connector` a connector
        // that is being stopped — then stop the handles the
        // unregistration handed back.
        connectors::unregister_and_stop_connectors(&mut self.connectors).await;
    }

    /// Attaches an already-registered connector to this running engine:
    /// resolves `name` in the process-wide connector registry, awaits
    /// the connector's `start` (bounded by the lifecycle timeout), and
    /// puts it under this engine's lifecycle management.
    ///
    /// This is the running-phase counterpart of
    /// [`add_connector`](SingleThreadedEngine::add_connector), reachable
    /// over the engine message channel
    /// ([`AddConnectorReq`](EngineMessage::AddConnectorReq)). Handles
    /// cannot ride in engine messages, so callers register the handle
    /// first and attach it here by name.
    ///
    /// A registered connector must be attached to at most one engine:
    /// the registry carries no ownership mark, so a second engine
    /// attaching the same name would double-start the connector and
    /// its reset would unregister the name out from under the first
    /// engine.
    ///
    /// # Errors
    ///
    /// Returns an error message if the name is not registered, is
    /// already engine-managed, or the connector fails to start (or
    /// times out). On a start failure the connector's `stop` is
    /// awaited before returning — a failing `start` may have spawned
    /// IO tasks before erroring (or completed after the timeout
    /// cancelled the await), and nothing else would ever stop an
    /// untracked connector. The name stays registered, so a retry
    /// remains possible.
    pub(crate) async fn attach_connector(&mut self, name: String) -> Result<String, String> {
        connectors::attach_connector(&mut self.connectors, name).await
    }

    /// Detaches an engine-managed connector: stops tracking it,
    /// unregisters it from the process-wide registry, and then awaits
    /// its `stop` — the same ordering as a reset, so no block can
    /// resolve a connector that is being stopped. Stop failures and
    /// timeouts are logged, not fatal.
    ///
    /// Reachable over the engine message channel
    /// ([`RemoveConnectorReq`](EngineMessage::RemoveConnectorReq)).
    ///
    /// # Errors
    ///
    /// Returns an error message if `name` is not engine-managed.
    pub(crate) async fn detach_connector(&mut self, name: &str) -> Result<String, String> {
        connectors::detach_connector(&mut self.connectors, name).await
    }

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
        // `spawn_block()`); it returns the assigned block id.
        schedule_block_on_engine(&block_def.desc, block_id, self)
    }

    pub(crate) async fn inspect_block(&self, id: &Uuid) -> Result<BlockDefinition, EngineError> {
        self.graph.inspect_block(id).await
    }

    pub(crate) async fn write_input(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Option<Value>, EngineError> {
        self.graph.write_input(id, name, value).await
    }

    pub(crate) async fn write_output(
        &self,
        id: &Uuid,
        name: String,
        value: Value,
    ) -> Result<Value, EngineError> {
        self.graph.write_output(id, name, value).await
    }

    /// Connects two blocks (source pin → target input). The source pin can
    /// be either an output or an input (the latter is input-fanout).
    pub(crate) async fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData> {
        self.graph.connect_blocks(link_data).await
    }

    pub(crate) async fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        self.graph.remove_block(block_id).await
    }

    /// Snapshot the full program: every scheduled block with its UI
    /// metadata + current pin values, plus every link. This is the
    /// canonical save format — round-trips through `load_program`.
    pub(crate) async fn save_program(&self) -> Result<Program> {
        self.graph.save_program().await
    }

    /// Load a full [`Program`], all or nothing: schedule every block,
    /// wire every link, push the saved input/output values, and store
    /// the UI metadata and the program's name and description.
    ///
    /// The whole program is checked before anything is scheduled — see
    /// `stage_program` for the checks; among them, the engine must not
    /// already hold any of the program's block or link ids (reset it
    /// first via [`Reset`](EngineMessage::Reset)). A failure after
    /// that, while wiring, removes the program's blocks again, so a
    /// failed load leaves the engine as it was.
    ///
    /// Running-phase only: the actor tasks need to be live to handle the
    /// WriteInput/Output mailbox cmds. Reached through the engine message
    /// channel ([`LoadProgramReq`](EngineMessage::LoadProgramReq)).
    pub(crate) async fn load_program(&mut self, program: Program) -> Result<()> {
        // Sync: check the program and schedule its blocks. After this
        // the per-block actor tasks have been spawned.
        let staged = stage_program(self, &program)?;
        self.graph.load_staged(staged).await
    }

    pub(crate) async fn disconnect_link_by_id(&mut self, link_id: &Uuid) -> Result<bool> {
        self.graph.disconnect_link_by_id(link_id).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;
    use crate::base::block::BlockProps;
    use crate::blocks::math::Add;

    /// Regression: reset awaited each `Terminate` send outside
    /// `run_until`, where no actor drains its mailbox, so one full
    /// mailbox would block the engine for good.
    #[tokio::test(flavor = "current_thread")]
    async fn reset_does_not_wait_on_a_full_mailbox() {
        let mut eng = SingleThreadedEngine::new();
        let add = Add::new();
        let id = *add.id();
        eng.schedule(add).expect("scheduled");
        let mut eng = eng.into_state::<Running>();

        // Nothing drives the LocalSet here, so nothing drains these.
        let mailbox = eng.graph.mailbox(&id).expect("scheduled").clone();
        while mailbox
            .try_send(BlockMailboxCmd::RefreshInput {
                name: "in0".to_string(),
            })
            .is_ok()
        {}

        timeout(Duration::from_secs(2), eng.reset())
            .await
            .expect("reset completes despite the full mailbox");
        assert!(eng.block_handles().is_empty());
    }
}
