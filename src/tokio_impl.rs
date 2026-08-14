// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;
use tokio::sync::watch::{Receiver, Sender};

use crate::base::Status;
use crate::base::block::Block;

pub mod block;
pub mod engine;
pub mod input;
pub mod output;
pub mod sleep;

/// The unit of data on a pin watch channel: the value plus the producer's
/// quality assertion. See [`crate::base::Status`] for the rationale.
pub type PinPayload = (Value, Status);

/// Tokio-based Reader: a watch-channel [`Receiver`] for [`PinPayload`]. The
/// channel coalesces — only the latest value is observed, which is what a
/// reactive dataflow engine wants. Edge-detection blocks track their own
/// previous state in the block struct rather than relying on every transition
/// being delivered.
pub type ReaderImpl = Receiver<PinPayload>;
/// Tokio-based Writer: the matching watch [`Sender`]. `send` overwrites the
/// current value and never blocks or fails on a "full" channel.
pub type WriterImpl = Sender<PinPayload>;

/// Trait-alias shorthand for "a [`Block`] using this crate's tokio
/// watch-channel reader/writer types". Equivalent to
/// [`Block`]`<Writer = `[`WriterImpl`]`, Reader = `[`ReaderImpl`]`>` but lets engine /
/// mailbox code state the bound concisely. Auto-implemented for any
/// matching [`Block`]. Allows [`?Sized`](Sized) so it can replace the bound in
/// trait-object-taking helpers like `add_output_link_inner`.
pub trait EngineBlock: Block<Writer = WriterImpl, Reader = ReaderImpl> {}
impl<T: Block<Writer = WriterImpl, Reader = ReaderImpl> + ?Sized> EngineBlock for T {}

/// Trait-alias shorthand for "an [`EngineBlock`] schedulable on the
/// multi-threaded engine". Adds [`Send`] + [`Sync`]:
/// - [`Send`] is required because the block crosses thread boundaries
///   into [`tokio::spawn`].
/// - [`Sync`] is required because the actor task holds `&block` across
///   `.await` points (the change-of-value RwLock read).
#[cfg(all(feature = "multi-threaded", not(target_arch = "wasm32")))]
pub trait MtBlock: EngineBlock + Send + Sync {}
#[cfg(all(feature = "multi-threaded", not(target_arch = "wasm32")))]
impl<T: EngineBlock + Send + Sync> MtBlock for T {}
