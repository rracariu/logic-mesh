// Copyright (c) 2022-2023, Radu Racariu.

//! Block execution engine.
//!
//! ## Engine phases
//!
//! An engine moves through two phases, tracked in its type:
//!
//! ```text
//! Engine<Idle> --run()--> Engine<Running> --Shutdown--> Engine<Idle>
//! ```
//!
//! - [`Idle`] — configuration. Blocks are scheduled, connectors added and
//!   message channels created; the [`Engine`] trait is implemented for
//!   this phase only.
//! - [`Running`] — the event loop. It exists only inside
//!   [`run`](Engine::run), which owns the engine for its whole duration,
//!   so the running-phase operations — the ones the engine's messages
//!   are routed to — cannot be reached from configuration code, and
//!   configuration cannot be applied to a running engine.
//!
//! [`run`](Engine::run) hands the idle engine back after
//! [`Shutdown`](messages::EngineMessage::Shutdown), so it can be
//! reconfigured and run again.
//!
//! ## Lifecycle messages
//!
//! Every engine follows one contract for the lifecycle
//! [`EngineMessage`](messages::EngineMessage)s. Blocks execute only
//! while the engine runs and is not paused: an idle engine — before its
//! first `run()` and after a `Shutdown` — holds every block, and a run
//! lets blocks execute only once it has started the managed connectors
//! and wired the links queued while idle.
//!
//! - [`Shutdown`](messages::EngineMessage::Shutdown) stops block
//!   execution and the managed connectors, then ends `run()`, which
//!   hands back the idle engine with every block, link and pin value
//!   intact and its connectors still registered. Running it again
//!   resumes the blocks where they stopped.
//! - [`Pause`](messages::EngineMessage::Pause) stops block execution:
//!   no block executes, so no output changes on its own, yet every
//!   request is still processed and answered as usual — pin writes and
//!   block, link and connector changes included. A value written while
//!   paused reaches the inputs linked to it, and their blocks react once
//!   execution resumes. Pausing a paused engine does nothing; connector
//!   IO tasks are not paused.
//! - [`Resume`](messages::EngineMessage::Resume) restarts block
//!   execution; resuming an engine that is not paused does nothing.
//!   Every `run()` starts unpaused.
//! - [`Reset`](messages::EngineMessage::Reset) terminates and removes
//!   every block and link, forgets the loaded program's name and
//!   description, and unregisters the managed connectors, whether or
//!   not the engine is paused. A paused engine stays paused.

use super::error::Result;
use super::{block::Block, program::Program};

pub mod messages;

mod sealed {
    pub trait Sealed {}
}

/// A phase of an engine's lifecycle: [`Idle`] or [`Running`].
///
/// Sealed — the engines implement their phase-specific APIs for these
/// two states only.
pub trait EngineState: sealed::Sealed {}

/// Configuration phase: the engine accepts blocks, connectors and message
/// channels, and has not started its event loop.
#[derive(Debug)]
pub enum Idle {}

/// Event-loop phase: the engine is servicing messages. Only exists inside
/// [`Engine::run`].
#[derive(Debug)]
pub enum Running {}

impl sealed::Sealed for Idle {}
impl sealed::Sealed for Running {}
impl EngineState for Idle {}
impl EngineState for Running {}

/// [`Send`] + [`Sync`] on native targets; no bound on `wasm32`.
///
/// This is the thread-safety a block needs to be scheduled through the
/// [`Engine`] trait: on native targets every engine — including the
/// multi-threaded one, which moves blocks across worker threads — can
/// host such a block. `wasm32` is single-threaded by definition, and its
/// JS-backed blocks are `!Send`, so the bound is dropped there.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSendSync: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> MaybeSendSync for T {}

/// [`Send`] + [`Sync`] on native targets; no bound on `wasm32`.
///
/// This is the thread-safety a block needs to be scheduled through the
/// [`Engine`] trait: on native targets every engine — including the
/// multi-threaded one, which moves blocks across worker threads — can
/// host such a block. `wasm32` is single-threaded by definition, and its
/// JS-backed blocks are `!Send`, so the bound is dropped there.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSendSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSendSync for T {}

/// Interface for an engine that implements block execution logic.
///
/// Implemented by engines in the [`Idle`] phase; see the
/// [module docs](self) for the phase model.
///
/// # Examples
///
/// ```no_run
/// use logic_mesh::{
///     base::engine::Engine,
///     blocks::math::Add,
///     single_threaded::SingleThreadedEngine,
/// };
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut engine = SingleThreadedEngine::new();
/// engine.schedule(Add::new())?;
/// // Resolves on `Shutdown`, handing the idle engine back.
/// let engine = engine.run().await;
/// # Ok(())
/// # }
/// ```
pub trait Engine: Sized {
    /// The transmission type of the blocks.
    type Writer;
    /// The reception type of the blocks.
    type Reader;

    /// The type used to send messages to/from this engine.
    type Channel: Send + Sync + Clone;

    /// Schedules a block to be executed by this engine.
    ///
    /// On native targets the block must be [`Send`] + [`Sync`] (see
    /// [`MaybeSendSync`]), so every engine can host it and a block the
    /// multi-threaded engine could not move across threads is rejected at
    /// compile time. The single-threaded engine can also host such blocks
    /// through its inherent
    /// [`schedule_local`](crate::single_threaded::SingleThreadedEngine::schedule_local).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::DuplicateBlockId`](crate::base::error::EngineError::DuplicateBlockId)
    /// if a block with the same id is already scheduled — a scheduled
    /// block is never replaced.
    ///
    /// # Examples
    ///
    /// A block holding a [`Cell`](std::cell::Cell) is not [`Sync`], so no
    /// engine accepts it here:
    ///
    /// ```compile_fail,E0277
    /// use std::cell::Cell;
    /// use logic_mesh::{
    ///     BlockProps, block,
    ///     base::{block::Block, engine::Engine},
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
    ///     async fn execute(&mut self) {}
    /// }
    ///
    /// let mut engine = SingleThreadedEngine::new();
    /// engine.schedule(Counter::new())?;
    /// # Ok::<(), logic_mesh::Error>(())
    /// ```
    fn schedule<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Writer = Self::Writer, Reader = Self::Reader> + MaybeSendSync + 'static;

    /// Synchronously schedules the blocks and validates-and-queues the links
    /// from a [`Program`]. The queued links are wired, and pin values
    /// can be pushed, once [`run`](Self::run) starts — to load a full
    /// program including its pin values, send a
    /// [`LoadProgramReq`](messages::EngineMessage::LoadProgramReq) to the
    /// running engine instead.
    ///
    /// # Errors
    ///
    /// Every block id is checked before any block is scheduled: an id
    /// that does not parse, or that is already scheduled or named twice
    /// in `program`
    /// ([`EngineError::DuplicateBlockId`](crate::base::error::EngineError::DuplicateBlockId)),
    /// rejects the program without scheduling any of its blocks. An
    /// unknown block type or an invalid link is reported after the
    /// blocks before it were scheduled.
    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()>;

    /// Runs the event loop of this engine and executes the scheduled blocks.
    ///
    /// Consumes the idle engine and drives it in the [`Running`] phase
    /// until a [`Shutdown`](messages::EngineMessage::Shutdown) message
    /// arrives, then returns it to the [`Idle`] phase so it can be
    /// reconfigured and run again. Blocks and the connectors the engine
    /// manages survive a shutdown, and the blocks resume executing on
    /// the next run; dropping the returned engine ends their actor tasks
    /// and unregisters the connectors. See the
    /// [lifecycle contract](self#lifecycle-messages).
    ///
    /// The returned future must be driven to completion — it is not a
    /// cancellation point. Dropping it mid-flight can leave a
    /// multi-step dispatch (link wiring, connector attach) half-applied,
    /// and drops the engine with it.
    #[allow(async_fn_in_trait)]
    async fn run(self) -> Self;

    /// Returns a handle to this engine's messaging system so external
    /// systems can communicate with this engine once it is running.
    ///
    /// Replies sent back on `sender_channel` carry no correlation id,
    /// so each channel supports one outstanding request at a time: send
    /// a request, then receive its reply before sending the next. A
    /// caller that abandons a receive after a completed send leaves
    /// that reply queued and desynchronizes every later reply on the
    /// channel. Replies are also delivered with a non-blocking send —
    /// a full channel (the capacity is whatever the caller created
    /// `sender_channel` with) drops the reply rather than stall
    /// the engine — so keep the channel drained.
    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel;
}
