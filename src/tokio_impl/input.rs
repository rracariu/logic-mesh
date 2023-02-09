// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::pin::Pin;

use futures::FutureExt;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::input::{BaseInput, Input, InputDefault, InputReceiver};

pub type InputImpl = BaseInput<Receiver<Value>, Sender<Value>>;

impl InputImpl {
    pub fn new(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
        Self::new_with_default(name, kind, block_id, Default::default())
    }

    pub fn new_with_default(
        name: &str,
        kind: HaystackKind,
        block_id: Uuid,
        default: InputDefault,
    ) -> Self {
        let (tx, rx) = channel::<Value>(32);

        Self {
            name: name.to_string(),
            kind,

            block_id,
            connection_count: 0,

            rx,
            tx,

            val: Default::default(),
            default,
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
    use uuid::Uuid;

    use crate::base::input::InputDefault;

    use super::InputImpl;

    #[test]
    fn test_input_init() {
        let input = InputImpl::new_with_default(
            "test",
            HaystackKind::Bool,
            Uuid::new_v4(),
            InputDefault {
                val: 0.into(),
                min: Value::Null,
                max: Value::Null,
            },
        );

        assert_eq!(input.name, "test");
        assert_eq!(input.kind, HaystackKind::Bool);
    }
}
