use tokio::sync::mpsc::{channel, Receiver, Sender};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::base::input::{BaseInput, Input, InputDefault, InputDesc};

pub type InputImpl = BaseInput<Receiver<Value>, Sender<Value>>;

impl InputImpl {
    pub fn new_without_default(name: &str, kind: HaystackKind) -> Self {
        Self::new(name, kind, Default::default())
    }

    pub fn new(name: &str, kind: HaystackKind, default: InputDefault) -> Self {
        let (tx, rx) = channel::<Value>(32);

        Self {
            desc: InputDesc {
                name: name.to_string(),
                kind,
            },
            default,
            rx,
            tx,
            val: Default::default(),
        }
    }
}

impl Input for InputImpl {
    type Rx = Receiver<Value>;
    type Tx = Sender<Value>;

    fn desc(&self) -> &InputDesc {
        &self.desc
    }

    fn default(&self) -> &InputDefault {
        &self.default
    }

    fn reader(&self) -> &Self::Rx {
        &self.rx
    }

    fn writer(&mut self) -> &mut Self::Tx {
        &mut self.tx
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::{kind::HaystackKind, Value};

    use crate::base::input::InputDefault;

    use super::InputImpl;

    #[test]
    fn test_input_init() {
        let input = InputImpl::new(
            "test",
            HaystackKind::Bool,
            InputDefault {
                val: 0.into(),
                min: Value::Null,
                max: Value::Null,
            },
        );

        assert_eq!(input.desc.name, "test");
        assert_eq!(input.desc.kind, HaystackKind::Bool);
    }
}
