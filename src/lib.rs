// Copyright (c) 2022-2023, Radu Racariu.

#[macro_use]
extern crate logic_mesh_block_macro;

// Lets the macro-generated `::logic_mesh::...` paths resolve inside this
// crate as well as in downstream crates.
extern crate self as logic_mesh;

pub use logic_mesh_block_macro::{BlockProps, block};

pub use libhaystack::val::kind::HaystackKind;
pub use uuid::Uuid;

pub mod base;
pub mod blocks;
mod tokio_impl;
pub use tokio_impl::engine::*;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
