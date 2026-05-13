// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;
use tokio::sync::watch::{Receiver, Sender};

use crate::base::Status;

pub mod block;
pub mod engine;
pub mod input;
pub mod output;
pub mod sleep;

/// The unit of data on a pin watch channel: the value plus the producer's
/// quality assertion. See [`crate::base::Status`] for the rationale.
pub type PinPayload = (Value, Status);

/// Tokio-based Reader: a watch-channel `Receiver` for [`PinPayload`]. The
/// channel coalesces — only the latest value is observed, which is what a
/// reactive dataflow engine wants. Edge-detection blocks track their own
/// previous state in the block struct rather than relying on every transition
/// being delivered.
pub type ReaderImpl = Receiver<PinPayload>;
/// Tokio-based Writer: the matching watch `Sender`. `send` overwrites the
/// current value and never blocks or fails on a "full" channel.
pub type WriterImpl = Sender<PinPayload>;
