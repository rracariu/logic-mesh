// Copyright (c) 2022-2024, Radu Racariu.

use libhaystack::val::kind::HaystackKind;
use libhaystack::val::{Bool, Value};

use crate::base::block::convert_value_kind;
use crate::base::input::InputProps;
use crate::base::output::Output;
use crate::blocks::{InputImpl, OutputImpl};

/// Sets `out` to `func` of the two inputs, `in2` converted to the kind
/// of `in1`. `in2` reads as missing while `in1` is, or when it does not
/// convert.
pub(super) fn compare(
    in1: &InputImpl,
    in2: &InputImpl,
    out: &mut OutputImpl,
    func: impl FnOnce(Option<&Value>, Option<&Value>) -> bool,
) {
    let value = match (in1.value(), in2.value()) {
        (Some(in1), Some(in2)) => {
            let (expected, actual) = (HaystackKind::from(in1), HaystackKind::from(in2));
            // The values `convert_value_kind` returns as they are, compared
            // without cloning them.
            if expected == actual || actual == HaystackKind::Null {
                func(Some(in1), Some(in2))
            } else {
                let in2 = convert_value_kind(in2.clone(), expected, actual).ok();
                func(Some(in1), in2.as_ref())
            }
        }
        (in1, _) => func(in1, None),
    };

    out.set(Bool { value }.into());
}

#[cfg(test)]
mod test {
    use libhaystack::val::Value;
    use libhaystack::val::kind::HaystackKind;
    use uuid::Uuid;

    use super::compare;
    use crate::base::output::OutputProps;
    use crate::blocks::{InputImpl, OutputImpl};

    fn compared(in1: Option<Value>, in2: Value) -> (Option<Value>, Option<Value>) {
        let block_id = Uuid::new_v4();
        let mut input1 = InputImpl::new("in1", HaystackKind::Null, block_id);
        let mut input2 = InputImpl::new("in2", HaystackKind::Null, block_id);
        let mut out = OutputImpl::new(HaystackKind::Bool, block_id);
        if let Some(in1) = in1 {
            input1.set(in1);
        }
        input2.set(in2);

        let mut seen = (None, None);
        compare(&input1, &input2, &mut out, |a, b| {
            seen = (a.cloned(), b.cloned());
            true
        });
        assert_eq!(out.value(), &Value::make_bool(true));
        seen
    }

    #[test]
    fn in2_is_converted_to_the_kind_of_in1() {
        assert_eq!(
            compared(Some(Value::make_int(2)), Value::make_str("2")),
            (Some(Value::make_int(2)), Some(Value::make_int(2)))
        );
        assert_eq!(
            compared(Some(Value::make_str("a")), Value::make_str("b")),
            (Some(Value::make_str("a")), Some(Value::make_str("b")))
        );
    }

    #[test]
    fn in2_reads_as_missing_when_it_does_not_convert_or_in1_is_missing() {
        assert_eq!(
            compared(Some(Value::make_int(2)), Value::make_str("not zinc (")),
            (Some(Value::make_int(2)), None)
        );
        assert_eq!(compared(None, Value::make_int(2)), (None, None));
    }
}
