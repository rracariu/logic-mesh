// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::Link;

use super::{props::OutDesc, OutputProps};

/// The base implementation of an output pin
#[derive(Debug)]
pub struct BaseOutput<L: Link> {
    desc: OutDesc,
    pub value: Value,
    pub links: Vec<L>,
    pub block_id: Uuid,
}

/// The implementation of the OutputProps trait
impl<L: Link> OutputProps for BaseOutput<L> {
    fn desc(&self) -> &OutDesc {
        &self.desc
    }

    fn block_id(&self) -> &Uuid {
        &self.block_id
    }

    fn value(&self) -> &Value {
        &self.value
    }

    fn is_connected(&self) -> bool {
        !self.links.is_empty()
    }

    fn links(&self) -> Vec<&dyn Link> {
        self.links.iter().map(|l| l as &dyn Link).collect()
    }

    fn remove_target_block(&mut self, block_id: &Uuid) {
        self.links.retain(|link| link.target_block_id() != block_id);
    }

    fn remove_all_links(&mut self) {
        self.links.clear();
    }
}

impl<L: Link> BaseOutput<L> {
    /// Creates a new output pin
    ///
    /// # Arguments
    /// * `name` - The name of the output pin
    /// * `kind` - The haystack kind of the output pin
    /// * `block_id` - The block id of the block this output pin belongs to
    ///
    /// # Returns
    /// A new output pin
    pub fn new_named(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
        Self {
            desc: OutDesc {
                name: name.to_string(),
                kind,
            },
            value: Value::default(),
            links: Vec::new(),
            block_id,
        }
    }

    /// Creates a new output pin with the name "out"
    /// # Arguments
    /// * `kind` - The haystack kind of the output pin
    /// * `block_id` - The block id of the block this output pin belongs to
    /// # Returns
    /// A new output pin
    pub fn new(kind: HaystackKind, block_id: Uuid) -> Self {
        Self::new_named("out", kind, block_id)
    }
}
