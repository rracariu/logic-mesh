// Copyright (c) 2022-2023, Radu Racariu.

//! Basic building blocks of the engine.

pub mod block;
pub mod connector;
pub mod engine;
// The errors' canonical path is the crate root (`logic_mesh::Error`, …).
pub(crate) mod error;
pub mod input;
pub mod link;
pub mod output;
pub mod program;
pub(crate) mod sealed;
pub(crate) mod status;

pub use status::Status;
