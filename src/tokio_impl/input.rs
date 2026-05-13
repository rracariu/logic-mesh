// Copyright (c) 2022-2023, Radu Racariu.

use std::pin::Pin;

use tokio::sync::watch;

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::{
    base::{
        Status,
        input::{BaseInput, Input, InputDefault},
    },
    tokio_impl::{PinPayload, ReaderImpl, WriterImpl},
};

pub type InputImpl = BaseInput<ReaderImpl, WriterImpl>;

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
        // Watch is a coalescing 1-slot channel: producers always succeed and
        // overwrite; consumers always see the latest value. The seed
        // `(Value::Null, Status::Ok)` is what `borrow()` reports until the
        // first real send.
        let (writer, reader) = watch::channel::<PinPayload>((Value::Null, Status::Ok));

        Self {
            name: name.to_string(),
            kind,

            block_id,
            connection_count: 0,

            reader,
            writer,

            val: Default::default(),
            status: Status::Ok,
            default,
            links: Default::default(),
        }
    }
}

impl Input for InputImpl {
    fn receiver(&mut self) -> Pin<Box<dyn Future<Output = Option<Value>> + Send + '_>> {
        // Wait for the next change. We deliberately do NOT consume the value
        // here: `changed()` marks the latest value as seen, so we re-arm via
        // `mark_changed()` so that the subsequent `try_take()` in
        // `drain_ready_inputs` actually surfaces it. Without re-arming, the
        // value would be silently dropped — `read_block_inputs` discards the
        // future result with `let _ = ...` and relies on `try_take` to
        // surface the value.
        Box::pin(async move {
            match self.reader.changed().await {
                Ok(()) => {
                    self.reader.mark_changed();
                    Some(self.reader.borrow().0.clone())
                }
                Err(_) => None,
            }
        })
    }

    fn try_take(&mut self) -> Option<(Value, Status)> {
        // Watch's `has_changed` returns `Err` only if every sender was
        // dropped — treat that as "no fresh value" rather than propagating.
        if self.reader.has_changed().unwrap_or(false) {
            Some(self.reader.borrow_and_update().clone())
        } else {
            None
        }
    }

    fn set_value(&mut self, value: Value, status: Status) {
        // Forward to all chained inputs. `send_if_modified` only notifies
        // when the value actually changed, so identical re-emissions don't
        // wake downstream blocks — see `Output::set` for the loop-quiescence
        // rationale. Status is part of the comparison so a producer's
        // transition Ok→Fault wakes downstream even when the value is the
        // same.
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                tx.send_if_modified(|current| {
                    if current.0 != value || current.1 != status {
                        current.0 = value.clone();
                        current.1 = status;
                        true
                    } else {
                        false
                    }
                });
            }
        }
        self.val = Some(value);
        self.status = status;
    }

    fn status(&self) -> Status {
        self.status
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
