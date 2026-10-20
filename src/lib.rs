// Copyright (c) 2022-2023, Radu Racariu.

#![doc = include_str!("../README.md")]

#[macro_use]
extern crate logic_mesh_block_macro;

extern crate self as logic_mesh;

pub use logic_mesh_block_macro::{BlockProps, block};

pub use libhaystack::val::Value;
pub use libhaystack::val::kind::HaystackKind;
pub use uuid::Uuid;

/// The [`libhaystack`] this crate is built against.
///
/// Its types appear in this crate's API beyond [`Value`] and
/// [`HaystackKind`] — units, numbers and the Zinc encoder's error that
/// [`ValueError::ZincEncode`] carries — so naming them through this
/// re-export keeps them the same version as the crate's own.
pub use libhaystack;

/// The [`serde_json`] this crate is built against.
///
/// A program's editor metadata
/// ([`EditorMetadata`](base::program::EditorMetadata)) is a
/// `serde_json` object, so building it through this re-export keeps it
/// the same version as the crate's own.
pub use serde_json;

/// The tokio channel modules the engines' API is built on.
///
/// An engine's [`Channel`](base::engine::Engine::Channel) is an
/// [`mpsc::Sender`](sync::mpsc::Sender), the watch subscription in
/// [`Messages`](base::engine::Messages) an
/// [`mpsc::UnboundedSender`](sync::mpsc::UnboundedSender),
/// and pins read and write [`watch`](sync::watch) channels
/// ([`blocks::ReaderImpl`], [`blocks::WriterImpl`]). Creating these
/// channels through this module keeps them the same tokio version as
/// the crate's own, without a direct tokio dependency for the
/// channels alone. The runtime that drives an engine remains the
/// application's choice.
pub mod sync {
    pub use tokio::sync::{mpsc, watch};
}

pub mod base;
pub mod blocks;
pub mod prelude;
mod tokio_impl;

pub use tokio_impl::engine::single_threaded;

#[cfg(all(feature = "multi-threaded", not(target_arch = "wasm32")))]
pub use tokio_impl::engine::multi_threaded;

/// The per-subsystem errors this crate reports, the [`Error`] aggregate
/// that wraps them, and the matching `Result` alias.
pub use base::error::{
    BlockDescError, ConnectError, ConnectorError, ConnectorErrorSource, EngineError, Error,
    ExternalError, LinkEnd, RegistryError, Result, ValueError,
};

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// Not public API: what the block macros' generated code calls.
#[doc(hidden)]
#[path = "macro_support.rs"]
pub mod __private;
