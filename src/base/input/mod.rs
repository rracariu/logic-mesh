// Copyright (c) 2022-2023, Radu Racariu.

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

/// The input trait
pub trait Input: InputProps {
    /// Gets this input receiver which can be polled for data.
    fn receiver(&mut self) -> Pin<Box<dyn Future<Output = Option<Value>> + Send + '_>>;

    /// Sets this input value
    fn set_value(&mut self, value: Value);
}
