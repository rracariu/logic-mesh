// Copyright (c) 2022-2023, Radu Racariu.

//! Block input properties trait.

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::link::{BaseLink, Link};

/// Basic properties of a block input.
pub trait InputProps {
    /// The input's read type.
    type Reader;
    /// The input's write type.
    type Writer: Clone;

    /// Returns the input's name.
    fn name(&self) -> &str;

    /// Returns the kind of data this input can receive.
    fn kind(&self) -> &HaystackKind;

    /// Returns the block id of the block this input belongs to.
    fn block_id(&self) -> &Uuid;

    /// Returns `true` if this input is connected to at least one output or input
    /// of another block.
    fn is_connected(&self) -> bool;

    /// Get a list of links to this output. The trait objects carry a
    /// `+` [`Send`] bound so they can be held across `.await` points in the
    /// MT actor future.
    fn links(&self) -> Vec<&(dyn Link + Send)>;

    /// Returns `true` if this input has at least one output.
    fn has_output(&self) -> bool {
        !self.links().is_empty()
    }

    /// Adds a link to this input.
    fn add_link(&mut self, link: BaseLink<Self::Writer>);

    /// Removes a link from this input.
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Removes a link by id from this input.
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Removes all links to a specific block from this input.
    fn remove_target_block_links(&mut self, block_id: &Uuid);

    /// Removes all links from this input.
    fn remove_all_links(&mut self);

    /// Returns a mutable reference to this input's reader.
    fn reader(&mut self) -> &mut Self::Reader;

    /// Returns a mutable reference to this input's writer.
    fn writer(&mut self) -> &mut Self::Writer;

    /// Returns the current value of this input.
    fn get_value(&self) -> Option<&Value>;

    /// Increments the connection count when this input
    /// is linked to another block's output.
    fn increment_conn(&mut self) -> usize;

    /// Decrements the connection count when the link
    /// to another block's output is removed.
    fn decrement_conn(&mut self) -> usize;
}
