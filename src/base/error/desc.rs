// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when a block description is read from text: the
//! [`FromStr`](std::str::FromStr) parsers of its enum fields, and the
//! conversion of a description a JavaScript host registers.
//!

use thiserror::Error;

/// Failures of reading a [`BlockDesc`](crate::base::block::BlockDesc)
/// field from a string, and of converting the description a
/// JavaScript host registers.
///
/// The conversion reports a bad field as [`Field`](Self::Field), which
/// names the field and carries what is wrong with it as its
/// [`source`](std::error::Error::source), and a bad pin kind as
/// [`UnknownPinKind`](Self::UnknownPinKind), which names the pin. Only
/// `wasm32` builds, where a JavaScript host registers blocks, produce
/// those two and [`NotExternal`](Self::NotExternal).
///
/// # Examples
///
/// ```
/// use logic_mesh::{BlockDescError, base::block::BlockRunCondition};
///
/// let err = "sometimes".parse::<BlockRunCondition>().expect_err("not a run condition");
/// assert_eq!(
///     err,
///     BlockDescError::UnknownRunCondition { value: "sometimes".to_string() }
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BlockDescError {
    /// The string names no
    /// [`BlockImplementation`](crate::base::block::BlockImplementation).
    #[error("Invalid implementation: {value}")]
    UnknownImplementation {
        /// The string that was rejected.
        value: String,
    },

    /// The string names no
    /// [`BlockRunCondition`](crate::base::block::BlockRunCondition).
    #[error("Invalid run condition: {value}")]
    UnknownRunCondition {
        /// The string that was rejected.
        value: String,
    },

    /// A pin of a JS-registered block names no Haystack kind. Kinds
    /// are the lowercase Haystack names (`"number"`, `"str"`, …).
    #[error("pin '{pin}': Invalid kind: {kind}")]
    UnknownPinKind {
        /// The pin whose kind was rejected.
        pin: String,
        /// The kind that was rejected.
        kind: String,
    },

    /// A field of a JS-registered block's description is invalid.
    #[error("field '{field}'")]
    Field {
        /// The field, as JavaScript names it, e.g. `runCondition`.
        field: &'static str,
        /// What is wrong with the field.
        #[source]
        error: Box<BlockDescError>,
    },

    /// A JS-registered block did not declare the `"external"`
    /// implementation. The JS function is the implementation, so a
    /// JS-registered block cannot be native.
    #[error("a JS-registered block must be 'external', got '{implementation}'")]
    NotExternal {
        /// The implementation the description declared.
        implementation: String,
    },
}

#[cfg(test)]
mod test {
    use std::error::Error as _;

    use super::BlockDescError;
    use crate::base::error::ErrorChain;

    /// Regression: `Field` printed what is wrong with the field in its
    /// own message and had no source, unlike every other wrapping error.
    #[test]
    fn field_reports_what_is_wrong_as_its_source() {
        let err = BlockDescError::Field {
            field: "runCondition",
            error: Box::new(BlockDescError::UnknownRunCondition {
                value: "sometimes".to_string(),
            }),
        };

        assert_eq!(err.to_string(), "field 'runCondition'");
        assert_eq!(
            err.source().map(|s| s.to_string()).as_deref(),
            Some("Invalid run condition: sometimes")
        );
        assert_eq!(
            ErrorChain(&err).to_string(),
            "field 'runCondition': Invalid run condition: sometimes"
        );
    }

    /// The messages JavaScript sees name the field as before.
    #[test]
    fn not_external_reads_as_a_field_error() {
        let err = BlockDescError::Field {
            field: "implementation",
            error: Box::new(BlockDescError::NotExternal {
                implementation: "native".to_string(),
            }),
        };

        assert_eq!(
            ErrorChain(&err).to_string(),
            "field 'implementation': a JS-registered block must be 'external', got 'native'"
        );
    }
}
