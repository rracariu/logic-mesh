// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;
use tokio::sync::mpsc::{Receiver, Sender};

pub mod block;
pub mod engine;
pub mod input;
pub mod output;
pub mod sleep;

/// Tokio based Reader with a MPSC Receiver for Haystack Value types.
pub type ReaderImpl = Receiver<Value>;
/// Tokio based Writer with a MPSC Sender for Haystack Value types.
pub type WriterImpl = Sender<Value>;
