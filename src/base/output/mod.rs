// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the base output types and traits.
//!

pub mod base;
pub mod props;

use libhaystack::val::Value;

use super::Status;
use super::link::BaseLink;

pub use base::BaseOutput;
pub use props::OutputProps;

pub trait Output: OutputProps {
    type Writer: Clone;

    /// Adds a link to this output
    fn add_link(&mut self, link: BaseLink<Self::Writer>);

    /// Set this output's value, paired with the output's current
    /// effective status, and broadcast it on all the output's registered
    /// links. The effective status is managed by the actor task via
    /// [`Output::emit_status`].
    fn set(&mut self, value: Value);

    /// Re-emit the current value with the given status. Used by the
    /// actor task to push a fault marker (or recovery) to downstream
    /// consumers without otherwise changing the value, and to update
    /// the effective status that subsequent [`Output::set`] calls use.
    fn emit_status(&mut self, status: Status);
}
