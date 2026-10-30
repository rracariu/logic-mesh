// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when a value is converted to the type a block pin
//! expects, or when numbers are brought to a common unit.
//!

use libhaystack::val::{Value, kind::HaystackKind};
use thiserror::Error;

/// Failures of the value and unit conversions performed on block pins.
///
/// A variant that wraps another error exposes it through
/// [`source`](std::error::Error::source) and leaves it out of its own
/// message, so a reporter that walks the error chain prints it once.
/// The fault reason of a block whose input fails to convert walks the
/// chain, so it includes the wrapped error.
///
/// [`ZincDecode`](Self::ZincDecode) has no `From<std::io::Error>`: an
/// I/O error is a Zinc decoding failure only where Zinc is decoded, so
/// `?` does not turn one into it.
///
/// ```compile_fail
/// let _: logic_mesh::ValueError = std::io::Error::other("disk").into();
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValueError {
    /// A value of `actual` kind cannot be converted to the kind a pin
    /// expects.
    #[error("Cannot convert {actual:?} to {expected:?}")]
    KindConversion {
        /// The kind the pin expects.
        expected: HaystackKind,
        /// The kind the value actually has.
        actual: HaystackKind,
    },

    /// A value did not hold the kind the conversion required.
    ///
    /// The value is boxed: it is by far the largest thing any variant
    /// carries, and inlining it would size every `Result<_, ValueError>`
    /// on the per-cycle conversion path by this cold error path.
    #[error("Expected a {expected:?} value, but got {actual:?}")]
    UnexpectedValue {
        /// The kind the conversion expected.
        expected: HaystackKind,
        /// The rejected value.
        actual: Box<Value>,
    },

    /// A value could not be converted to the requested Haystack type.
    ///
    /// Carries `libhaystack`'s `ConversionError`, which is a
    /// `&'static str`, so this costs no allocation. It is a message,
    /// not an error, so it is part of this error's message.
    #[error("Value conversion failed: {0}")]
    Conversion(&'static str),

    /// Two numbers could not be brought to a common unit. The detail is
    /// a message, not an error, so it is part of this error's message.
    #[error("Unit conversion failed: {0}")]
    UnitConversion(String),

    /// Decoding a Zinc-encoded string failed. `libhaystack` surfaces
    /// Zinc decoding failures as [`std::io::Error`], the source.
    #[error("Zinc decoding failed")]
    ZincDecode(#[source] std::io::Error),

    /// Encoding a value as a Zinc string failed.
    #[error("Zinc encoding failed")]
    ZincEncode(#[from] libhaystack::encoding::zinc::encode::Error),

    /// A string was not a valid boolean literal.
    #[error("Invalid boolean literal")]
    ParseBool(#[from] std::str::ParseBoolError),
}

#[cfg(test)]
mod test {
    use std::error::Error as _;

    use super::ValueError;

    /// The messages of an error and of every source after it, as a
    /// chain-walking reporter prints them.
    fn chain(err: &dyn std::error::Error) -> Vec<String> {
        let mut messages = vec![err.to_string()];
        let mut source = err.source();
        while let Some(err) = source {
            messages.push(err.to_string());
            source = err.source();
        }
        messages
    }

    /// Regression: the wrapping variants printed their source in their
    /// own message too, so a reporter walking the chain printed it twice.
    #[test]
    fn wrapped_errors_are_reported_once() {
        let decode = ValueError::ZincDecode(std::io::Error::other("bad token"));
        assert_eq!(chain(&decode), ["Zinc decoding failed", "bad token"]);

        let parse = ValueError::from("maybe".parse::<bool>().expect_err("not a bool"));
        let messages = chain(&parse);
        assert_eq!(messages[0], "Invalid boolean literal");
        assert!(parse.source().is_some());
        assert!(!messages[0].contains(&messages[1]));
    }

    /// Converting a string that is not Zinc reports the decoder's error
    /// as the source.
    #[test]
    fn zinc_decode_failure_keeps_the_decoder_error() {
        let err = crate::base::block::convert_value_kind(
            libhaystack::val::Value::make_str("not zinc ("),
            libhaystack::val::kind::HaystackKind::Number,
            libhaystack::val::kind::HaystackKind::Str,
        )
        .expect_err("not a number");

        assert!(matches!(err, ValueError::ZincDecode(_)));
        assert!(err.source().is_some());
    }
}
