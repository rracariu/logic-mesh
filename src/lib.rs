// Copyright (c) 2022-2023, IntriSemantics Corp.

#![allow(incomplete_features)]
#![feature(async_closure)]
#![feature(async_fn_in_trait)]
#![feature(trait_alias)]
#![feature(trait_upcasting)]

#[macro_use]
extern crate block_macro;

pub mod base;
pub mod blocks;
mod tokio_impl;
pub use tokio_impl::engine::*;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
