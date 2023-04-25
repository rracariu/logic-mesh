// Copyright (c) 2022-2023, IntriSemantics Corp.

pub mod base;
pub mod props;

use libhaystack::val::Value;
use uuid::Uuid;

use super::link::{BaseLink, Link};

pub use base::BaseOutput;
pub use props::OutputProps;

pub trait Output: OutputProps {
    type Writer: Clone;

    /// Adds a link to this output
    fn add_link(&mut self, link: BaseLink<Self::Writer>);

    /// Remove a link from this output
    /// # Arguments
    /// - link: The link to be removed
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Remove a link by id from this output
    /// # Arguments
    /// - link_id: The id of the link to be removed
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Set this output's value by
    /// sending this value to all the registered links
    /// of this output.
    fn set(&mut self, value: Value);
}
