// Copyright (c) 2022-2026, Radu Racariu.

//! [`SingleThreadedEngine`] — engine struct, lifecycle, and the
//! synchronous + async API surface that messaging dispatch routes to.

use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, rc::Rc, time::Duration};

use crate::base::error::{EngineError, Result};
use tokio::{
    sync::{
        mpsc::{self, Receiver},
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
        messages::{ClientId, RequestMessage, RequestSender, ResponseSender, WatchId, WatchSender},
    },
    program::{EditorMetadata, Program, data::Position},
};
use crate::blocks::registry::BlockSink;
use crate::tokio_impl::engine::message_dispatch::{Clients, Flow, RunningHost, dispatch};
use crate::tokio_impl::engine::{
    REQUEST_CHANNEL_CAP, ensure_schedulable, phase_name, schedule_block_on_engine,
};
use crate::tokio_impl::scan::ScanInterval;
use crate::tokio_impl::{ReaderImpl, WriterImpl};
use futures::future::LocalBoxFuture;

/// Single-threaded execution environment for blocks.
///
/// The type parameter is the engine's lifecycle phase — see the
/// [phase model](crate::base::engine#engine-phases). Configure an
/// [`Idle`] engine, then [`run`](Engine::run) it.
///
/// # Examples
///
/// The engine runs its blocks on the thread that drives it, so a
/// current-thread runtime (tokio's `rt` and `macros` features) is all it
/// needs.
///
/// ```no_run
/// use logic_mesh::{
///     base::block::connect_output,
///     blocks::{math::Add, misc::SineWave},
///     prelude::*,
///     single_threaded::SingleThreadedEngine,
/// };
///
/// #[tokio::main(flavor = "current_thread")]
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
    /// The request channel every client sends on; the engine keeps a
    /// sender to hand out to new clients.
    sender: RequestSender,
    receiver: Receiver<RequestMessage>,
    /// The clients' response channels.
    clients: Clients,
    /// Watchers for change-of-value notifications, by watch id.
    watchers: Rc<RefCell<BTreeMap<WatchId, WatchSender>>>,
    /// Names of the connectors whose lifecycle this engine manages. The
    /// handles themselves live in the process-wide connector registry —
    /// the single source of truth blocks also resolve against. Dropping
    /// the engine unregisters them.
    connectors: ManagedConnectors,
    /// Execution gate every block actor subscribes to: `true` while the
    /// engine runs and is not paused. Actors keep serving their mailbox
    /// either way, so requests are answered while blocks are held.
    execution: watch::Sender<bool>,
    /// The scan interval every block actor task runs with.
    scan: ScanInterval,
    /// The lifecycle phase, tracked in the type only.
    state: PhantomData<S>,
}

impl Default for SingleThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Shows the engine's phase, whether its blocks may execute, how many
/// blocks, links and staged programs it holds, the connectors it
/// manages and its scan interval — not the blocks or channels
/// themselves.
impl<S: EngineState> std::fmt::Debug for SingleThreadedEngine<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SingleThreadedEngine")
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

impl Engine for SingleThreadedEngine<Idle> {
    type Writer = WriterImpl;
    type Reader = ReaderImpl;
    type RunFuture = LocalBoxFuture<'static, Self>;

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

    fn add_connector(&mut self, name: &str, handle: impl Into<ConnectorHandle>) -> Result<()> {
        register_connector(name, handle)?;
        self.connectors.push(name.to_string());
        Ok(())
    }

    fn managed_connectors(&self) -> Vec<String> {
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

    fn set_block_editor(&mut self, id: &Uuid, editor: Option<EditorMetadata>) -> Result<()> {
        Ok(self.graph.set_editor(id, editor)?)
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

    fn create_message_channel(&mut self, responses: ResponseSender) -> (ClientId, RequestSender) {
        (self.clients.register(responses), self.sender.clone())
    }
}

impl SingleThreadedEngine<Idle> {
    /// Creates a new single-threaded engine.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(REQUEST_CHANNEL_CAP);
        Self {
            local: Rc::new(LocalSet::new()),
            graph: BlockGraph::default(),
            pending: Vec::new(),
            sender,
            receiver,
            clients: Clients::default(),
            watchers: Rc::default(),
            connectors: ManagedConnectors::default(),
            execution: watch::Sender::new(false),
            scan: ScanInterval::default(),
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
    /// use logic_mesh::{prelude::*, single_threaded::SingleThreadedEngine};
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
            clients,
            watchers,
            connectors,
            execution,
            scan,
            state: _,
        } = self;
        SingleThreadedEngine {
            local,
            graph,
            pending,
            sender,
            receiver,
            clients,
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
        self.local.spawn_local(self.scan.scope(block_actor_task(
            block,
            mailbox_rx,
            watchers,
            self.execution.subscribe(),
        )));
        Ok(())
    }

    /// Validates `link_data` against the scheduled blocks — see
    /// `BlockGraph::validate_link`.
    #[cfg(test)]
    pub(super) fn validate_link(
        &self,
        link_data: &crate::base::program::LinkData,
    ) -> Result<(Uuid, LinkRecord)> {
        self.graph.validate_link(link_data)
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

    fn schedule_desc(&mut self, desc: &BlockDesc, id: Option<Uuid>) -> Result<Uuid> {
        schedule_block_on_engine(desc, id, self)
    }
}

impl SingleThreadedEngine<Running> {
    /// The event loop: starts the managed connectors, wires the links
    /// and pushes the pin values of the programs scheduled while idle,
    /// opens the execution gate, then answers engine requests until a
    /// [`Shutdown`](crate::base::engine::messages::EngineRequest::Shutdown) — see the
    /// [lifecycle contract](crate::base::engine#lifecycle-requests).
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
            let mut request = None;
            local
                .run_until(async {
                    request = self.receiver.recv().await;
                })
                .await;

            // The engine holds a sender itself, so the channel never
            // closes.
            let Some(request) = request else {
                continue;
            };
            if local.run_until(dispatch(self, request)).await == Flow::Shutdown {
                break;
            }
        }
    }
}

impl RunningHost for SingleThreadedEngine<Running> {
    fn clients(&mut self) -> &mut Clients {
        &mut self.clients
    }

    fn connectors(&mut self) -> &mut ManagedConnectors {
        &mut self.connectors
    }

    fn set_execution(&self, open: bool) {
        SingleThreadedEngine::set_execution(self, open);
    }

    async fn add_watcher(&mut self, id: WatchId, sender: WatchSender) {
        self.watchers.borrow_mut().insert(id, sender);
    }

    async fn remove_watcher(&mut self, id: WatchId) -> bool {
        self.watchers.borrow_mut().remove(&id).is_some()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;
    use crate::base::block::BlockProps;
    use crate::base::engine::messages::{EngineRequest, EngineResponse, Envelope, RequestId};
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
        let (responses, mut answered) = mpsc::unbounded_channel();
        let (client, _) = eng.create_message_channel(responses);
        let mut eng = eng.into_state::<Running>();

        // Nothing drives the LocalSet here, so nothing drains these.
        let mailbox = eng.graph.mailbox(&id).expect("scheduled").clone();
        while mailbox
            .try_send(BlockMailboxCmd::RefreshInput {
                name: "in0".to_string(),
            })
            .is_ok()
        {}

        let reset = Envelope::new(client, RequestId(0), EngineRequest::Reset);
        timeout(Duration::from_secs(2), dispatch(&mut eng, reset))
            .await
            .expect("reset completes despite the full mailbox");
        assert!(eng.graph.handles().is_empty());
        let ack = answered.try_recv().expect("the reset is answered");
        assert!(matches!(ack.body, EngineResponse::Reset { result: Ok(()) }));
    }
}
