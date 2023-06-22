// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::Link;

/// The description of an output pin
#[derive(Debug, Default, Clone)]
pub struct OutDesc {
    pub name: String,
    pub kind: HaystackKind,
}

/// Properties of a block output pin
pub trait OutputProps {
    /// The output's description
    fn desc(&self) -> &OutDesc;

    /// The output's name
    fn name(&self) -> &str {
        &self.desc().name
    }

    /// The block id of the block this output belongs to
    fn block_id(&self) -> &Uuid;

    /// True if this output is connected to at least one input
    fn is_connected(&self) -> bool;

    /// Get a list of links to this output
    fn links(&self) -> Vec<&dyn Link>;

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

    /// Remove all links to a specific block from this output
    fn remove_target_block_links(&mut self, block_id: &Uuid);

    /// Remove all links from this output
    fn remove_all_links(&mut self);

    /// Get this output's value
    fn value(&self) -> &Value;
}
