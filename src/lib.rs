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
mod tokio_impl;
pub use tokio_impl::engine::*;

/// The per-subsystem errors this crate reports, the [`Error`] aggregate
/// that wraps them, and the matching `Result` alias.
pub use base::error::{
    EngineError, Error, ExternalError, LinkEnd, RegistryError, Result, ValueError,
};

#[cfg(target_arch = "wasm32")]
pub mod wasm;
