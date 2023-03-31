// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::Link;

#[derive(Debug)]
pub struct OutDesc {
    pub name: String,
    pub kind: HaystackKind,
}

/// Properties of a block output pin
pub trait OutputProps {
    /// The output's description
    fn desc(&self) -> &OutDesc;

    /// The block id of the block this output belongs to
    fn block_id(&self) -> &Uuid;

    /// True if this output is connected to at least one input
    fn is_connected(&self) -> bool;

    /// Get a list of links to this output
    fn links(&self) -> Vec<&dyn Link>;

    /// Get this output's value
    fn value(&self) -> &Value;
}
