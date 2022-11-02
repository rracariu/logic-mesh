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
    type Tx;

    fn desc(&self) -> &InputDesc;

    fn default(&self) -> &InputDefault;

    fn reader(&self) -> &Self::Rx;

    fn writer(&mut self) -> &mut Self::Tx;
}

pub struct BaseInput<Rx, Tx> {
    pub desc: InputDesc,
    pub default: InputDefault,
    pub rx: Rx,
    pub tx: Tx,
    pub val: Option<Value>,
}
