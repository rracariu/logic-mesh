// Copyright (c) 2026, Radu Racariu.

//! The pin changes a watch notification carries, shared by both
//! engines' actor loops.

use libhaystack::val::Value;

use crate::base::block::Block;
use crate::base::engine::messages::ChangeSource;

/// The pin values a block's watchers were last told about, by pin
/// position: an input and an output may share a name, so a name does
/// not identify a pin.
#[derive(Debug, Default)]
pub(super) struct PinSnapshot {
    outputs: Vec<Option<Value>>,
    inputs: Vec<Option<Value>>,
}

impl PinSnapshot {
    /// Forgets every value, so that the next [`changes`](Self::changes)
    /// reports every pin that has one: what a watcher subscribing later
    /// needs first.
    pub(super) fn clear(&mut self) {
        self.outputs.clear();
        self.inputs.clear();
    }

    /// The pins whose value differs from the snapshot, outputs first and
    /// each in the block's pin order, taking their values into the
    /// snapshot. An input that has no value yet has not changed.
    pub(super) fn changes<B: Block>(&mut self, block: &B) -> Vec<ChangeSource> {
        let mut changes = Vec::new();

        let outputs = block.outputs();
        self.outputs.resize(outputs.len(), None);
        for (last, output) in self.outputs.iter_mut().zip(outputs) {
            let value = output.value();
            if last.as_ref() != Some(value) {
                *last = Some(value.clone());
                changes.push(ChangeSource::Output(
                    output.name().to_string(),
                    value.clone(),
                ));
            }
        }

        let inputs = block.inputs();
        self.inputs.resize(inputs.len(), None);
        for (last, input) in self.inputs.iter_mut().zip(inputs) {
            if let Some(value) = input.value()
                && last.as_ref() != Some(value)
            {
                *last = Some(value.clone());
                changes.push(ChangeSource::Input(input.name().to_string(), value.clone()));
            }
        }

        changes
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use assert_matches::assert_matches;
    use libhaystack::val::Value;

    use super::PinSnapshot;
    use crate::base::Status;
    use crate::base::block::BlockProps;
    use crate::base::engine::messages::ChangeSource;
    use crate::base::output::Output;
    use crate::blocks::math::Add;
    use crate::tokio_impl::engine::test_support::runtime_descs::Echo;

    #[test]
    fn reports_only_what_changed_since_the_last_call() {
        let mut block = Add::new();
        let mut snapshot = PinSnapshot::default();

        assert_matches!(
            &snapshot.changes(&block)[..],
            [ChangeSource::Output(name, Value::Null)] if name == "out"
        );
        assert!(snapshot.changes(&block).is_empty());

        block.inputs_mut()[1].set_value(2.into(), Status::Ok);
        block.out.set(2.into());
        assert_matches!(
            &snapshot.changes(&block)[..],
            [ChangeSource::Output(out, out_value), ChangeSource::Input(input, in_value)]
                if out == "out" && *out_value == 2.into()
                    && input == block.inputs()[1].name() && *in_value == 2.into()
        );
        assert!(snapshot.changes(&block).is_empty());

        snapshot.clear();
        assert_eq!(snapshot.changes(&block).len(), 2);
    }

    /// Regression: changes were keyed by pin name, so when an input and
    /// an output shared one, a notification carried only one of them.
    #[test]
    fn an_input_and_an_output_sharing_a_name_are_both_reported() {
        let mut block = Echo::with_pin_names("value", "value");
        let mut snapshot = PinSnapshot::default();
        snapshot.changes(&block);

        block.inputs_mut()[0].set_value(1.into(), Status::Ok);
        block.outputs_mut()[0].set(2.into());

        assert_matches!(
            &snapshot.changes(&block)[..],
            [ChangeSource::Output(out, out_value), ChangeSource::Input(input, in_value)]
                if out == "value" && *out_value == 2.into()
                    && input == "value" && *in_value == 1.into()
        );
        assert!(snapshot.changes(&block).is_empty());
    }
}
