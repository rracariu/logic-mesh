// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when a value is converted to the type a block pin
//! expects, or when numbers are brought to a common unit.
//!

use libhaystack::val::{Value, kind::HaystackKind};
use thiserror::Error;

/// Failures of the value and unit conversions performed on block pins.
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
    /// `&'static str`, so this costs no allocation.
    #[error("Value conversion failed: {0}")]
    Conversion(&'static str),

    /// Two numbers could not be brought to a common unit.
    #[error("Unit conversion failed: {0}")]
    UnitConversion(String),

    /// Decoding a Zinc-encoded string failed. `libhaystack` surfaces
    /// Zinc decoding failures as [`std::io::Error`].
    #[error("Zinc decoding failed: {0}")]
    ZincDecode(#[from] std::io::Error),

    /// Encoding a value as a Zinc string failed.
    #[error("Zinc encoding failed: {0}")]
    ZincEncode(#[from] libhaystack::encoding::zinc::encode::Error),

    /// A string was not a valid boolean literal.
    #[error("Invalid boolean literal: {0}")]
    ParseBool(#[from] std::str::ParseBoolError),
}
