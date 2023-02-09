// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::pin::Pin;

use futures::Future;
use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

/// A default set of values for an `Input`
#[derive(Debug, Default)]
pub struct InputDefault {
    pub val: Value,
    pub min: Value,
    pub max: Value,
}

///
/// Defines the basic properties of a Block Input
///
pub trait InputProps {
    type Rx;
    type Tx: Clone;

    /// The input's name
    fn name(&self) -> &str;

    /// The kind of data this input can receive
    fn kind(&self) -> &HaystackKind;

    /// The block id of the block this input belongs to
    fn block_id(&self) -> &Uuid;

    /// True if this input is connected to at least one output
    /// of another block
    fn is_connected(&self) -> bool;

    /// This input's defaults
    fn default(&self) -> &InputDefault;

    /// Get a reference to this input reader type
    fn reader(&mut self) -> &mut Self::Rx;

    /// Get a reference to this input writer type
    fn writer(&mut self) -> &mut Self::Tx;

    /// Sets this input value
    fn set_value(&mut self, value: Value);

    /// Increment the connection count when this input
    /// is linked to another block's output.
    fn increment_conn(&mut self) -> usize;

    /// Decrement the connection count when the link
    /// to another block output is removed.
    fn decrement_conn(&mut self) -> usize;
}

/// Type used to describe the receiver of an Input
pub trait InputReceiver = Future<Output = Option<Value>> + Send;

/// The Input type
pub trait Input: InputProps {
    /// Gets this input receiver which can be polled for data.
    fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>>;
}

#[derive(Debug, Default)]
pub struct BaseInput<Rx, Tx> {
    pub name: String,
    pub kind: HaystackKind,
    pub block_id: Uuid,
    pub connection_count: usize,
    pub rx: Rx,
    pub tx: Tx,
    pub val: Option<Value>,
    pub default: InputDefault,
}

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
        self.connection_count > 0
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
