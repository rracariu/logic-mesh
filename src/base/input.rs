use libhaystack::val::Value;

pub trait Input {
    type Rx;
    type Tx;

    fn init(&mut self);

    fn reader(&self) -> &Option<Self::Rx>;

    fn writer(&mut self) -> &mut Option<Self::Tx>;
}

pub struct BaseInput<Rx, Tx> {
    pub rx: Option<Rx>,
    pub tx: Option<Tx>,
    pub val: Option<Value>,
}

impl<Rx, Tx> Default for BaseInput<Rx, Tx> {
    fn default() -> Self {
        Self {
            rx: Default::default(),
            tx: Default::default(),
            val: Default::default(),
        }
    }
}
