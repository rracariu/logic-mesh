// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::Link;

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
    type Rx;
    type Tx: Clone;

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

    /// This input's defaults
    fn default(&self) -> &InputDefault;

    /// Get a reference to this input reader type
    fn reader(&mut self) -> &mut Self::Rx;

    /// Get a reference to this input writer type
    fn writer(&mut self) -> &mut Self::Tx;

    /// Gets this input value
    fn get_value(&self) -> &Option<Value>;

    /// Sets this input value
    fn set_value(&mut self, value: Value);

    /// Increment the connection count when this input
    /// is linked to another block's output.
    fn increment_conn(&mut self) -> usize;

    /// Decrement the connection count when the link
    /// to another block output is removed.
    fn decrement_conn(&mut self) -> usize;
}
