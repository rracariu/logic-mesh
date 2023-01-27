// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::pin::Pin;

use futures::FutureExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::base::input::{BaseInput, Input, InputDefault, InputDesc, InputReceiver};

pub type InputImpl = BaseInput<Receiver<Value>, Sender<Value>>;

impl InputImpl {
    pub fn new(name: &str, kind: HaystackKind) -> Self {
        Self::new_with_default(name, kind, Default::default())
    }

    pub fn new_with_default(name: &str, kind: HaystackKind, default: InputDefault) -> Self {
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
    fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>> {
        self.rx.recv().boxed()
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::{kind::HaystackKind, Value};

    use crate::base::input::InputDefault;

    use super::InputImpl;

    #[test]
    fn test_input_init() {
        let input = InputImpl::new_with_default(
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
