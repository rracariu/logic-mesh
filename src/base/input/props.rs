// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block input properties trait
//!

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::{BaseLink, Link};

/// A default set of values for an `Input`
#[derive(Debug, Default)]
pub struct InputDefault {
    /// The default value
    pub val: Value,
    /// The default minimum value
    pub min: Value,
    /// The default maximum value
    pub max: Value,
}

/// Defines the basic properties of a Block Input
pub trait InputProps {
    /// The input's read type
    type Reader;
    /// The input's write type
    type Writer: Clone;

    /// The input's name
    fn name(&self) -> &str;

    /// The kind of data this input can receive
    fn kind(&self) -> &HaystackKind;

    /// The block id of the block this input belongs to
    fn block_id(&self) -> &Uuid;

    /// True if this input is connected to at least one output or input
    /// of another block
    fn is_connected(&self) -> bool;

    /// Get a list of links to this output
    fn links(&self) -> Vec<&dyn Link>;

    /// True if this input has at least one output
    fn has_output(&self) -> bool {
        !self.links().is_empty()
    }

    /// Adds a link to this output
    fn add_link(&mut self, link: BaseLink<Self::Writer>);

    /// Remove a link from this input
    /// # Arguments
    /// - link: The link to be removed
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Remove a link by id from this input
    /// # Arguments
    /// - link_id: The id of the link to be removed
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Remove all links to a specific block from this input
    fn remove_target_block_links(&mut self, block_id: &Uuid);

    /// Remove all links from this input
    fn remove_all_links(&mut self);

    /// This input's defaults
    fn default(&self) -> &InputDefault;

    /// Get a reference to this input reader type
    fn reader(&mut self) -> &mut Self::Reader;

    /// Get a reference to this input writer type
    fn writer(&mut self) -> &mut Self::Writer;

    /// Gets this input value
    fn get_value(&self) -> Option<&Value>;

    /// Increment the connection count when this input
    /// is linked to another block's output.
    fn increment_conn(&mut self) -> usize;

    /// Decrement the connection count when the link
    /// to another block output is removed.
    fn decrement_conn(&mut self) -> usize;
}
