// Copyright (c) 2022-2023, IntriSemantics Corp.

pub mod base;
pub mod props;

use libhaystack::val::Value;

use super::link::{BaseLink, Link};

pub use base::BaseOutput;
pub use props::OutputProps;

pub trait Output: OutputProps {
    type Tx: Clone;

    /// Adds a link to this output
    fn add_link(&mut self, link: BaseLink<Self::Tx>);

    /// Remove a link from this output
    fn remove_link(&mut self, link: &dyn Link);

    /// Set this output's value by
    /// sending this value to all the registered links
    /// of this output.
    fn set(&mut self, value: Value);
}
