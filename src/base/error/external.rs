// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised for external (host-provided) blocks — blocks whose
//! executor is supplied by the JavaScript host rather than compiled in.
//!

use thiserror::Error;

/// Failures of external block resolution and execution.
///
/// External blocks only exist on `wasm32`, where the host registers the
/// JavaScript function that executes them; the `Unsupported*` variants
/// are what every other target and the multi-threaded engine report
/// instead.
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
    #[cfg(target_arch = "wasm32")]
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
    #[cfg(target_arch = "wasm32")]
    #[error("External block '{name}' factory function failed: {detail}")]
    FactoryCallFailed {
        /// The block whose factory threw.
        name: String,
        /// The error message from JavaScript.
        detail: String,
    },

    /// The JavaScript factory function returned something that is not a
    /// function, so there is nothing to execute the block with.
    #[cfg(target_arch = "wasm32")]
    #[error("External block '{name}' factory did not return a function: {detail}")]
    FactoryReturnedNonFunction {
        /// The block whose factory returned a non-function.
        name: String,
        /// Description of the unexpected return value.
        detail: String,
    },
}
