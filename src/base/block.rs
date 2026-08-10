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
    val::{Bool, Number, Str, Value, kind::HaystackKind},
};
pub use props::{BlockInput, BlockOutput, BlockProps};
use uuid::Uuid;

/// Operational state a block is in.
///
/// Phase 1 fault propagation: `Fault` carries the reason so the engine and
/// UI can show *why* the block isn't producing trustworthy output. Recovery
/// is automatic — the actor task optimistically clears Fault at the start
/// of each cycle and re-enters it only if drain or execute set it again.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum BlockState {
    /// Block is executing normally.
    #[default]
    Running,
    /// Block has detected an error condition. The reason describes the
    /// proximate cause (upstream fault, type conversion failure,
    /// block-specific computation failure). Downstream blocks that drain
    /// a Fault-status value transition to Fault themselves.
    Fault { reason: String },
    /// Block exists but is intentionally not executing. Phase 1 doesn't
    /// have a path that sets this; reserved for explicit user disable.
    Disabled,
    /// Block has been removed and the actor task is exiting.
    Terminated,
}

impl BlockState {
    /// Construct a Fault state with the given reason.
    pub fn fault(reason: impl Into<String>) -> Self {
        BlockState::Fault {
            reason: reason.into(),
        }
    }

    pub fn is_fault(&self) -> bool {
        matches!(self, BlockState::Fault { .. })
    }

    /// The fault reason, if in Fault state.
    pub fn fault_reason(&self) -> Option<&str> {
        match self {
            BlockState::Fault { reason } => Some(reason.as_str()),
            _ => None,
        }
    }

    /// Short label suitable for UI / inspect output.
    pub fn label(&self) -> &'static str {
        match self {
            BlockState::Running => "running",
            BlockState::Fault { .. } => "fault",
            BlockState::Disabled => "disabled",
            BlockState::Terminated => "terminated",
        }
    }
}

/// A block: a unit of dataflow logic that reacts to its inputs and
/// produces output values.
///
/// On native targets we declare `execute` as returning an `impl Future
/// + Send`. That promise lets the multi-threaded engine spawn block
/// actor tasks directly via [`tokio::spawn`], which requires the
/// future be `Send`. Macro-generated native blocks all satisfy this.
///
/// On `wasm32` (single-threaded by definition), we drop the `Send`
/// requirement so `JsBlock` — whose `func: js_sys::Function` is
/// `!Send` — can still implement the trait.
#[cfg(not(target_arch = "wasm32"))]
pub trait Block: BlockConnect {
    fn execute(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

#[cfg(target_arch = "wasm32")]
pub trait Block: BlockConnect {
    #[allow(async_fn_in_trait)]
    async fn execute(&mut self);
}

/// Construct a block instance with a fixed id.
///
/// Implemented by the `BlockProps` derive. The registry uses it to
/// instantiate runtime-registered blocks when a program prescribes the
/// block ids.
pub trait BlockConstruct: Sized {
    fn with_uuid(uuid: Uuid) -> Self;
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
    if expected == actual || actual == HaystackKind::Null {
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

            if val.value == "true" || val.value == "false" {
                return Ok(val.value.parse::<bool>()?.into());
            }

            let num = zinc::decode::from_str(&val.value)?;
            match num {
                Value::Number(Number { value, unit: _ }) => Ok((value != 0.0).into()),
                Value::Bool(Bool { value }) => Ok(value.into()),
                _ => Err(anyhow::anyhow!("Expected a bool value, but got {:?}", val)),
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

    use crate::base::block::{Block, BlockProps, BlockState};

    use crate::blocks::{InputImpl, OutputImpl, ReaderImpl, WriterImpl};

    use libhaystack::val::{Value, kind::HaystackKind};

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
        let test_block = &Test::new() as &dyn BlockProps<Reader = ReaderImpl, Writer = WriterImpl>;

        assert_eq!(test_block.desc().name, "Test");
        assert_eq!(test_block.desc().dis, "Test long name");
        assert_eq!(test_block.desc().library, "test");
        assert_eq!(test_block.state(), BlockState::Running);
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

        assert!(
            test_block
                .inputs()
                .iter()
                .filter(|input| input.name().starts_with("in"))
                .enumerate()
                .all(|(i, input)| input.name() == format!("in{}", i))
        );

        assert!(
            test_block
                .inputs()
                .iter()
                .all(|i| i.kind() == &HaystackKind::Number)
        );

        assert!(test_block.outputs()[0].desc().name == "out");
        assert!(test_block.outputs()[0].desc().kind == HaystackKind::Number);
        assert!(!test_block.outputs()[0].is_connected());
    }

    #[test]
    fn test_block_outputs() {
        let test_block = &Test::new() as &dyn BlockProps<Reader = ReaderImpl, Writer = WriterImpl>;

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
