// Copyright (c) 2022-2023, Radu Racariu.

#![allow(incomplete_features)]
#![feature(trait_alias)]
#![feature(assert_matches)]

#[macro_use]
extern crate logic_mesh_block_macro;

pub mod base;
pub mod blocks;
mod tokio_impl;
pub use tokio_impl::engine::*;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
