// Copyright (c) 2022-2026, Radu Racariu.

//! Multi-threaded engine.
//!
//! ## Architecture
//!
//! Each scheduled block is hosted by its own task spawned via
//! [`tokio::spawn`] onto the caller's multi-thread runtime. The runtime's
//! work-stealing scheduler decides which worker thread runs each task and
//! can migrate them between threads — block actor futures are [`Send`] by
//! construction (the [`Block`](crate::base::block::Block) trait's
//! `execute` return carries `+ Send` on native targets).
//!
//! The per-block mailbox protocol (`BlockMailboxCmd`) is shared with the
//! [`SingleThreadedEngine`]; the only MT-specific bits are watchers
//! (`Arc<RwLock<…>>` instead of `Rc<RefCell<…>>`) and the cross-thread
//! mailbox semantics.
//!
//! ### Cancellation safety
//!
//! The same guarantees as the [`SingleThreadedEngine`] apply: dropping
//! `block.execute()` mid-poll is safe because every standard-library
//! block structures `execute` as "wait on inputs, then synchronous logic."
//! The only awaits are at the input-wait stage, and the watch-channel
//! reads used there keep the value pending when cancelled — the next
//! cycle drains it from `try_take`.
//!
//! ## Module layout
//!
//! - `actor` — the per-block actor task (`block_actor_task`) and
//!   MT-specific watcher types.
//! - [`engine`] — [`MultiThreadedEngine`] itself: lifecycle, the
//!   [`Engine`] trait impl, sync configuration helpers, async APIs, and
//!   message dispatch.
//!
//! [`Engine`]: crate::base::engine::Engine
//! [`SingleThreadedEngine`]: crate::single_threaded::SingleThreadedEngine

mod actor;
pub mod engine;

pub use engine::{BlockHandle, Messages, MultiThreadedEngine};
