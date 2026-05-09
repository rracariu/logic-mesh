// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;
use tokio::sync::watch::{Receiver, Sender};

pub mod block;
pub mod engine;
pub mod input;
pub mod output;
pub mod sleep;

/// Tokio-based Reader: a watch-channel `Receiver` for Haystack `Value`s. The
/// channel coalesces — only the latest value is observed, which is what a
/// reactive dataflow engine wants. Edge-detection blocks track their own
/// previous state in the block struct rather than relying on every transition
/// being delivered.
pub type ReaderImpl = Receiver<Value>;
/// Tokio-based Writer: the matching watch `Sender`. `send` overwrites the
/// current value and never blocks or fails on a "full" channel.
pub type WriterImpl = Sender<Value>;
