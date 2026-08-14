// Copyright (c) 2022-2023, Radu Racariu.

//! Base output properties.

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::link::Link;

/// The description of an output pin.
#[derive(Debug, Default, Clone)]
pub struct OutDesc {
    /// The output's name.
    pub name: String,
    /// The output's haystack kind.
    pub kind: HaystackKind,
}

/// Properties of a block output pin.
pub trait OutputProps {
    /// Returns the output's description.
    fn desc(&self) -> &OutDesc;

    /// Returns the output's name.
    fn name(&self) -> &str {
        &self.desc().name
    }

    /// Returns the block id of the block this output belongs to.
    fn block_id(&self) -> &Uuid;

    /// Returns `true` if this output is connected to at least one input.
    fn is_connected(&self) -> bool;

    /// Get a list of links to this output. The trait objects carry a
    /// `+` [`Send`] bound so they can be held across `.await` points in the
    /// MT actor future.
    fn links(&self) -> Vec<&(dyn Link + Send)>;

    /// Removes a link from this output.
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Removes a link by id from this output.
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Removes all links to a specific block from this output.
    fn remove_target_block_links(&mut self, block_id: &Uuid);

    /// Removes all links from this output.
    fn remove_all_links(&mut self);

    /// Returns this output's value.
    fn value(&self) -> &Value;
}
