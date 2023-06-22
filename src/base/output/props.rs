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

    fn remove_target_block(&mut self, block_id: &Uuid);

    /// Remove all links from this output
    fn remove_all_links(&mut self);

    /// Get this output's value
    fn value(&self) -> &Value;
}
