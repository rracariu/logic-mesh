// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when a block description is read from text: the
//! [`FromStr`](std::str::FromStr) parsers of its enum fields, and the
//! conversion of a description a JavaScript host registers.
//!

use thiserror::Error;

/// Failures of reading a [`BlockDesc`](crate::base::block::BlockDesc)
/// field from a string.
///
/// # Examples
///
/// ```
/// use logic_mesh::{BlockDescError, base::block::desc::BlockRunCondition};
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
    /// [`BlockImplementation`](crate::base::block::desc::BlockImplementation).
    #[error("Invalid implementation: {value}")]
    UnknownImplementation {
        /// The string that was rejected.
        value: String,
    },

    /// The string names no
    /// [`BlockRunCondition`](crate::base::block::desc::BlockRunCondition).
    #[error("Invalid run condition: {value}")]
    UnknownRunCondition {
        /// The string that was rejected.
        value: String,
    },

    /// A pin of a JS-registered block names no Haystack kind. Kinds
    /// are the lowercase Haystack names (`"number"`, `"str"`, …).
    #[cfg(target_arch = "wasm32")]
    #[error("pin '{pin}': Invalid kind: {kind}")]
    UnknownPinKind {
        /// The pin whose kind was rejected.
        pin: String,
        /// The kind that was rejected.
        kind: String,
    },

    /// A field of a JS-registered block's description is invalid. The
    /// message names the field as the JS caller spells it, followed by
    /// what is wrong with it.
    #[cfg(target_arch = "wasm32")]
    #[error("field '{field}': {error}")]
    Field {
        /// The field, as JavaScript names it, e.g. `runCondition`.
        field: &'static str,
        /// What is wrong with the field. It is part of this error's
        /// message, so it is not also its source.
        error: Box<BlockDescError>,
    },

    /// A JS-registered block did not declare the `"external"`
    /// implementation. The JS function is the implementation, so a
    /// JS-registered block cannot be native.
    #[cfg(target_arch = "wasm32")]
    #[error(
        "field 'implementation': a JS-registered block must be 'external', got '{implementation}'"
    )]
    NotExternal {
        /// The implementation the description declared.
        implementation: String,
    },
}
