// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the base input type
//!

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::{BaseLink, Link};

use super::{props::InputDefault, InputProps};

/// The base input type
#[derive(Debug, Default)]
pub struct BaseInput<Rx, Tx> {
    /// The block unique input's name
    pub name: String,
    /// The kind of data this input can receive
    pub kind: HaystackKind,
    /// The block id of the block this input belongs to
    pub block_id: Uuid,
    /// The number of connections this input has
    pub connection_count: usize,
    /// The input reader
    pub rx: Rx,
    /// The input writer
    pub tx: Tx,
    /// The input value
    pub val: Option<Value>,
    /// The input default values
    pub default: InputDefault,
    /// The links to other inputs
    pub links: Vec<BaseLink<Tx>>,
}

/// Implements the `InputProps` trait for `BaseInput`
impl<Rx, Tx: Clone> InputProps for BaseInput<Rx, Tx> {
    type Rx = Rx;
    type Tx = Tx;

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
        self.connection_count > 0 || !self.links.is_empty()
    }

    fn links(&self) -> Vec<&dyn Link> {
        self.links.iter().map(|l| l as &dyn Link).collect()
    }

    fn add_link(&mut self, link: BaseLink<Self::Tx>) {
        self.links.push(link)
    }

    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.links.retain(|l| l.id() != link_id)
    }

    fn default(&self) -> &InputDefault {
        &self.default
    }

    fn reader(&mut self) -> &mut Self::Rx {
        &mut self.rx
    }

    fn writer(&mut self) -> &mut Self::Tx {
        &mut self.tx
    }

    fn get_value(&self) -> &Option<Value> {
        &self.val
    }

    fn set_value(&mut self, value: Value) {
        self.val = Some(value)
    }

    fn increment_conn(&mut self) -> usize {
        self.connection_count += 1;
        self.connection_count
    }

    fn decrement_conn(&mut self) -> usize {
        if self.connection_count > 1 {
            self.connection_count -= 1;
        }
        self.connection_count
    }
}
