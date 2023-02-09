// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::pin::Pin;

use futures::Future;
use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct InputDefault {
    pub val: Value,
    pub min: Value,
    pub max: Value,
}
pub trait InputProps {
    type Rx;
    type Tx: Clone;

    fn name(&self) -> &str;

    fn kind(&self) -> &HaystackKind;

    fn block_id(&self) -> &Uuid;

    fn increment_conn(&mut self) -> usize;

    fn decrement_conn(&mut self) -> usize;

    fn is_connected(&self) -> bool;

    fn default(&self) -> &InputDefault;

    fn reader(&mut self) -> &mut Self::Rx;

    fn writer(&mut self) -> &mut Self::Tx;

    fn set_value(&mut self, value: Value);
}
pub trait InputReceiver = Future<Output = Option<Value>> + Send;
pub trait Input: InputProps {
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
}
