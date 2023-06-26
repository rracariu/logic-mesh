// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the base input type
//!

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::{BaseLink, Link};

use super::{props::InputDefault, InputProps};

/// The base input type
#[derive(Debug, Default)]
pub struct BaseInput<Reader, Writer> {
    /// The block unique input's name
    pub name: String,
    /// The kind of data this input can receive
    pub kind: HaystackKind,
    /// The block id of the block this input belongs to
    pub block_id: Uuid,
    /// The number of connections this input has
    pub connection_count: usize,
    /// The input reader
    pub reader: Reader,
    /// The input writer
    pub writer: Writer,
    /// The input value
    pub val: Option<Value>,
    /// The input default values
    pub default: InputDefault,
    /// The links to other inputs
    pub links: Vec<BaseLink<Writer>>,
}

/// Implements the `InputProps` trait for `BaseInput`
impl<Reader, Writer: Clone> InputProps for BaseInput<Reader, Writer> {
    type Reader = Reader;
    type Writer = Writer;

    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &HaystackKind {
        &self.kind
    }

    fn block_id(&self) -> &Uuid {
        &self.block_id
    }

    fn is_connected(&self) -> bool {
        self.connection_count > 0
    }

    fn links(&self) -> Vec<&dyn Link> {
        self.links.iter().map(|l| l as &dyn Link).collect()
    }

    fn add_link(&mut self, link: BaseLink<Self::Writer>) {
        self.links.push(link)
    }

    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.links.retain(|l| l.id() != link_id)
    }

    fn remove_target_block_links(&mut self, block_id: &Uuid) {
        self.links.retain(|l| l.target_block_id() != block_id)
    }

    fn remove_all_links(&mut self) {
        self.links.clear()
    }

    fn default(&self) -> &InputDefault {
        &self.default
    }

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.reader
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }

    fn get_value(&self) -> &Option<Value> {
        &self.val
    }

    fn increment_conn(&mut self) -> usize {
        self.connection_count += 1;
        self.connection_count
    }

    fn decrement_conn(&mut self) -> usize {
        if self.connection_count > 0 {
            self.connection_count -= 1;
        }
        self.connection_count
    }
}
