// Copyright (c) 2022-2023, Radu Racariu.

use std::pin::Pin;

use futures::FutureExt;
use tokio::sync::mpsc::{Receiver, Sender, channel};

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::{
    input::{BaseInput, Input, InputDefault, InputReceiver},
    link::LinkState,
};

pub type Reader = Receiver<Value>;
pub type Writer = Sender<Value>;
pub type InputImpl = BaseInput<Reader, Writer>;

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
        let (writer, reader) = channel::<Value>(32);

        Self {
            name: name.to_string(),
            kind,

            block_id,
            connection_count: 0,

            reader,
            writer,

            val: Default::default(),
            default,
            links: Default::default(),
        }
    }
}

impl Input for InputImpl {
    fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>> {
        self.reader.recv().boxed()
    }

    fn set_value(&mut self, value: Value) {
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                if let Err(__) = tx.try_send(value.clone()) {
                    link.state = LinkState::Error;
                } else {
                    link.state = LinkState::Connected;
                }
            }
        }
        self.val = Some(value);
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::{Value, kind::HaystackKind};
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
