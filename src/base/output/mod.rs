// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the base output types and traits.
//!

pub mod base;
pub mod props;

use libhaystack::val::Value;

use super::link::BaseLink;

pub use base::BaseOutput;
pub use props::OutputProps;

pub trait Output: OutputProps {
    type Writer: Clone;

    /// Adds a link to this output
    fn add_link(&mut self, link: BaseLink<Self::Writer>);

    /// Set this output's value by
    /// sending this value to all the registered links
    /// of this output.
    fn set(&mut self, value: Value);
}
