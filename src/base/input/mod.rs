// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block input trait
//!

use std::pin::Pin;

use futures::Future;
use libhaystack::val::Value;

pub mod base;
pub mod input_reader;
pub mod props;

pub use base::BaseInput;
pub use props::{InputDefault, InputProps};

/// Type used to describe the receiver of an Input
pub trait InputReceiver = Future<Output = Option<Value>> + Send;

/// The input trait
pub trait Input: InputProps {
    /// Gets this input receiver which can be polled for data.
    fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>>;

    /// Sets this input value
    fn set_value(&mut self, value: Value);
}
