// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block trait and associated types
//!

pub mod connect;
pub mod desc;
pub mod props;

use anyhow::Result;
pub use connect::BlockConnect;
pub use desc::{BlockDesc, BlockPin, BlockStaticDesc};
use libhaystack::{
    encoding::zinc,
    val::{kind::HaystackKind, Bool, Number, Str, Value},
};
pub use props::BlockProps;

/// Determines the state a block is in
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    #[default]
    Stopped,
    Running,
    Fault,
    Terminate,
}

pub trait Block: BlockConnect {
    #[allow(async_fn_in_trait)]
    async fn execute(&mut self);
}

/// Converts the actual value to the expected type expected value.
///
/// # Arguments
/// - `expect` The expected value, this is used to determine the expected type
/// - `actual` The actual value to convert
///
/// # Returns
/// The converted value if the conversion was successful.
/// If the conversion was not successful, an error is returned.
pub fn convert_value(expect: &Value, actual: Value) -> Result<Value> {
    let to_kind = HaystackKind::from(&actual);
    convert_value_kind(actual, HaystackKind::from(expect), to_kind)
}

/// Converts a value from one kind to another.
///
/// # Arguments
/// - `val` The value to convert
/// - `expected` The expected kind of the value
/// - `actual` The actual kind of the value
///
/// # Returns
/// The converted value if the conversion was successful.
pub fn convert_value_kind(
    val: Value,
    expected: HaystackKind,
    actual: HaystackKind,
) -> Result<Value> {
    if expected == actual {
        return Ok(val);
    }

    match (expected, actual) {
        (HaystackKind::Bool, HaystackKind::Bool) => Ok(val),
        (HaystackKind::Bool, HaystackKind::Number) => {
            let val = Number::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            Ok((val.value != 0.0).into())
        }
        (HaystackKind::Bool, HaystackKind::Str) => {
            let val = Str::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            let num = zinc::decode::from_str(&val.value)?;
            if num.is_bool() {
                Ok(num)
            } else {
                Err(anyhow::anyhow!("Expected a bool value, but got {:?}", val))
            }
        }

        (HaystackKind::Number, HaystackKind::Number) => Ok(val),
        (HaystackKind::Number, HaystackKind::Bool) => {
            let val = Bool::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            Ok((if val.value { 1 } else { 0 }).into())
        }
        (HaystackKind::Number, HaystackKind::Str) => {
            let val = Str::try_from(&val).map_err(|err| anyhow::anyhow!(err))?;

            let num = zinc::decode::from_str(&val.value)?;
            if num.is_number() {
                Ok(num)
            } else {
                Err(anyhow::anyhow!(
                    "Expected a number value, but got {:?}",
                    val
                ))
            }
        }

        (HaystackKind::Str, HaystackKind::Str) => Ok(val),
        (HaystackKind::Str, HaystackKind::Bool) => Ok(val.to_string().as_str().into()),
        (HaystackKind::Str, HaystackKind::Number) => {
            let str = zinc::encode::to_zinc_string(&val)?;
            Ok(str.as_str().into())
        }

        (HaystackKind::Str, _) => {
            let str = zinc::encode::to_zinc_string(&val)?;
            Ok(str.as_str().into())
        }

        _ => Err(anyhow::anyhow!(
            "Cannot convert {:?} to {:?}",
            actual,
            expected
        )),
    }
}

#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps},
        output::Output,
    };

    use super::test_utils::mock::{InputImpl, OutputImpl};

    use libhaystack::val::{kind::HaystackKind, Value};

    #[block]
    #[derive(BlockProps, Debug)]
    #[dis = "Test long name"]
    #[library = "test"]
    #[category = "test"]
    #[input(kind = "Number", count = 16)]
    struct Test {
        #[input(kind = "Number")]
        user_defined: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }

    impl Block for Test {
        async fn execute(&mut self) {
            self.out.value = Value::make_int(42);
        }
    }

    #[test]
    fn test_block_props_declared_inputs() {
        let test_block = &Test::new() as &dyn BlockProps<Reader = String, Writer = String>;

        assert_eq!(test_block.desc().name, "Test");
        assert_eq!(test_block.desc().dis, "Test long name");
        assert_eq!(test_block.desc().library, "test");
        assert_eq!(test_block.state(), BlockState::Stopped);
        assert_eq!(test_block.inputs().len(), 17);
        assert_eq!(test_block.outputs().len(), 1);

        assert_eq!(
            test_block
                .inputs()
                .iter()
                .filter(|input| input.name().starts_with("in"))
                .count(),
            16
        );

        assert!(test_block
            .inputs()
            .iter()
            .filter(|input| input.name().starts_with("in"))
            .enumerate()
            .all(|(i, input)| input.name() == format!("in{}", i)));

        assert!(test_block
            .inputs()
            .iter()
            .all(|i| i.kind() == &HaystackKind::Number));

        assert!(test_block.outputs()[0].desc().name == "out");
        assert!(test_block.outputs()[0].desc().kind == HaystackKind::Number);
        assert!(!test_block.outputs()[0].is_connected());
    }

    #[test]
    fn test_block_outputs() {
        let test_block = &Test::new() as &dyn BlockProps<Reader = String, Writer = String>;

        assert_eq!(test_block.outputs().len(), 1);
        assert_eq!(test_block.outputs()[0].desc().name, "out");
        assert_eq!(test_block.outputs()[0].desc().kind, HaystackKind::Number);
        assert!(!test_block.outputs()[0].is_connected());
    }

    #[test]
    fn convert_value_num_to_bool_test() {
        let val = Value::make_bool(true);
        let converted =
            super::convert_value_kind(val, HaystackKind::Number, HaystackKind::Bool).unwrap();
        assert_eq!(converted, Value::make_int(1));
    }

    #[test]
    fn convert_value_str_to_num_test() {
        let val = Value::make_str("42");
        let converted =
            super::convert_value_kind(val, HaystackKind::Number, HaystackKind::Str).unwrap();
        assert_eq!(converted, Value::make_int(42));
    }
}
