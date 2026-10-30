// Copyright (c) 2022-2023, Radu Racariu.

use std::pin::Pin;

use tokio::sync::watch;

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::{
    base::{
        Status,
        input::{BaseInput, Input},
    },
    tokio_impl::{PinPayload, ReaderImpl, WriterImpl},
};

/// Concrete input type backed by tokio watch channels.
pub type InputImpl = BaseInput<ReaderImpl, WriterImpl>;

impl InputImpl {
    /// Creates a new input with a fresh watch channel.
    pub fn new(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
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
            links: Default::default(),
        }
    }

    /// Sets this input's value, with [`Status::Ok`], and forwards it to
    /// the inputs this one links to — [`Input::set_value`] with an `Ok`
    /// status.
    ///
    /// Use it to give an unconnected input a constant before the block
    /// is scheduled. A connected input takes the next value its link
    /// delivers instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use logic_mesh::{base::input::InputProps, blocks::misc::SineWave};
    ///
    /// let mut sine = SineWave::new();
    /// sine.amplitude.set(3);
    /// assert_eq!(sine.amplitude.value(), Some(&3.into()));
    /// ```
    pub fn set(&mut self, value: impl Into<Value>) {
        Input::set_value(self, value.into(), Status::Ok);
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
        // Early-return on no-op: if neither value nor status changed
        // relative to this input's cached state, every chained channel
        // already has the same payload — `send_if_modified` would be a
        // no-op on each, but we still pay for the value clones in the
        // closure capture. Skip the loop entirely. This makes the
        // semantics explicit: `set_value` only propagates on actual
        // change.
        if self.val.as_ref() == Some(&value) && self.status == status {
            return;
        }

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

    use super::InputImpl;
    use crate::base::{
        Status,
        block::connect_input,
        input::{Input, InputProps},
        output::{Output, OutputProps},
    };
    use crate::tokio_impl::output::OutputImpl;

    #[test]
    fn test_input_init() {
        let input = InputImpl::new("test", HaystackKind::Bool, Uuid::new_v4());

        assert_eq!(input.name, "test");
        assert_eq!(input.kind, HaystackKind::Bool);
    }

    /// `set` gives the input its value and forwards it like a value
    /// that arrived on a link.
    #[test]
    fn set_stores_and_forwards_the_value() {
        let mut source = InputImpl::new("in", HaystackKind::Number, Uuid::new_v4());
        let mut target = InputImpl::new("in", HaystackKind::Number, Uuid::new_v4());
        connect_input(&mut source, &mut target).expect("not linked yet");
        assert_eq!(target.connection_count(), 1);

        source.set(3);

        assert_eq!(source.value(), Some(&3.into()));
        assert_eq!(source.status(), Status::Ok);
        assert_eq!(target.try_take(), Some((3.into(), Status::Ok)));
    }

    /// Regression: the input accessor was `InputProps::get_value` while
    /// the output one is `OutputProps::value`, so block code read the
    /// two pin kinds through different names. Both are `value` now.
    #[test]
    fn inputs_and_outputs_read_their_value_by_the_same_name() {
        fn input_value<I: InputProps>(input: &I) -> Option<&Value> {
            input.value()
        }
        fn output_value<O: OutputProps>(output: &O) -> &Value {
            output.value()
        }

        let block_id = Uuid::new_v4();
        let mut input = InputImpl::new("in", HaystackKind::Number, block_id);
        let mut output = OutputImpl::new(HaystackKind::Number, block_id);
        assert_eq!(input_value(&input), None);

        input.set(3);
        output.set(3.into());

        assert_eq!(input_value(&input), Some(output_value(&output)));
    }
}
