// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block input trait
//!

use std::pin::Pin;

use futures::Future;
use libhaystack::val::Value;

use crate::base::Status;

pub mod base;
pub mod input_reader;
pub mod props;

pub use base::BaseInput;
pub use props::{InputDefault, InputProps};

/// The input trait
pub trait Input: InputProps {
    /// Gets this input receiver which can be polled for data.
    fn receiver(&mut self) -> Pin<Box<dyn Future<Output = Option<Value>> + Send + '_>>;

    /// Non-blocking peek: take the latest (value, status) if a fresh
    /// payload has arrived since the last observation, returning `None`
    /// if nothing fresh is available. Used by `read_block_inputs` to
    /// drain every input that has new data in a single cycle, rather
    /// than one input per cycle.
    fn try_take(&mut self) -> Option<(Value, Status)>;

    /// Set this input's cached value and status, and forward the same
    /// payload to any chained input links. `status` carries the upstream
    /// producer's quality assertion through input-fanout chains.
    fn set_value(&mut self, value: Value, status: Status);

    /// The status of the last value seen on this input. Defaults to
    /// `Status::Ok` for inputs that have never received a payload.
    fn status(&self) -> Status;
}
