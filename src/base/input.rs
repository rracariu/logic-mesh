use std::pin::Pin;

use futures::Future;
use libhaystack::val::{kind::HaystackKind, Value};

pub struct InputDesc {
    pub name: String,
    pub kind: HaystackKind,
}

#[derive(Debug, Default)]
pub struct InputDefault {
    pub val: Value,
    pub min: Value,
    pub max: Value,
}

pub trait Input {
    type Rx;
    type Tx: Clone;

    fn desc(&self) -> &InputDesc;

    fn default(&self) -> &InputDefault;

    fn reader(&mut self) -> &mut Self::Rx;

    fn writer(&mut self) -> &mut Self::Tx;

    fn set_value(&mut self, value: Value);
}
pub trait InputReceiver: Input {
    fn receiver(&mut self) -> Pin<Box<dyn Future<Output = Option<Value>> + Send + '_>>;
}

pub struct BaseInput<Rx, Tx> {
    pub desc: InputDesc,
    pub default: InputDefault,
    pub rx: Rx,
    pub tx: Tx,
    pub val: Option<Value>,
}

impl<Rx, Tx: Clone> Input for BaseInput<Rx, Tx> {
    type Rx = Rx;
    type Tx = Tx;

    fn desc(&self) -> &InputDesc {
        &self.desc
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
