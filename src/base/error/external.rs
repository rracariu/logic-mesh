// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised for external (host-provided) blocks — blocks whose
//! executor is supplied by the JavaScript host rather than compiled in —
//! and for the connectors that host supplies.
//!

use thiserror::Error;

/// Failures of external block resolution and execution, and of reading
/// a connector the JavaScript host supplies.
///
/// External blocks only exist on `wasm32` with the `js` feature, where
/// the host registers the JavaScript function that executes them; the
/// `Unsupported*` variants are what every other build and the
/// multi-threaded engine report instead. The other variants are
/// declared on every target, so code that handles them builds anywhere,
/// but only such a `wasm32` build produces them.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExternalError {
    /// This build has no host to supply external block executors.
    #[error("External blocks not supported on this platform")]
    Unsupported,

    /// External blocks cannot run on the multi-threaded engine.
    #[error("External blocks not supported in multi-threaded mode")]
    UnsupportedMultiThreaded,

    /// The JavaScript library that provides an external block's executor
    /// function is not registered with the engine.
    #[error(
        "Missing library: '{library}'. Can't find the executor JavaScript function for: '{name}' block."
    )]
    LibraryNotFound {
        /// The JavaScript library name that was not found.
        library: String,
        /// The block that requires the missing library.
        name: String,
    },

    /// The JavaScript factory function of an external block threw.
    #[error("External block '{name}' factory function failed: {detail}")]
    FactoryCallFailed {
        /// The block whose factory threw.
        name: String,
        /// The error message from JavaScript.
        detail: String,
    },

    /// The JavaScript factory function returned something that is not a
    /// function, so there is nothing to execute the block with.
    #[error("External block '{name}' factory did not return a function: {detail}")]
    FactoryReturnedNonFunction {
        /// The block whose factory returned a non-function.
        name: String,
        /// Description of the unexpected return value.
        detail: String,
    },

    /// Reading a method property of a JavaScript connector object
    /// threw, e.g. from a getter.
    #[error("Failed to read connector property '{property}': {detail}")]
    ConnectorPropertyUnreadable {
        /// The property that could not be read.
        property: String,
        /// The error message from JavaScript.
        detail: String,
    },

    /// A JavaScript connector object lacks a method it must have
    /// (`subscribe`, `publish` or `request`), or has a non-function in
    /// its place.
    #[error("Connector property '{property}' is required and must be a function")]
    ConnectorMethodMissing {
        /// The required property.
        property: String,
    },

    /// An optional method of a JavaScript connector object (`start` or
    /// `stop`) is present but not a function.
    #[error("Connector property '{property}', when present, must be a function")]
    ConnectorMethodNotFunction {
        /// The optional property.
        property: String,
    },
}
