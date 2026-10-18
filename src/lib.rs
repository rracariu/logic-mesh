// Copyright (c) 2022-2023, Radu Racariu.

#![doc = include_str!("../README.md")]

#[macro_use]
extern crate logic_mesh_block_macro;

extern crate self as logic_mesh;

pub use logic_mesh_block_macro::{BlockProps, block};

pub use libhaystack::val::Value;
pub use libhaystack::val::kind::HaystackKind;
pub use uuid::Uuid;

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
