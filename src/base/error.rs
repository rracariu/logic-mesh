// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Defines the errors reported by this crate.
//!
//! Each subsystem owns its own error enum — [`RegistryError`],
//! [`EngineError`], [`ValueError`] and [`ExternalError`] — so a failure
//! carries the data of the thing that failed and can be matched on
//! without inspecting a formatted message. [`Error`] is the aggregate
//! that the crate's entry points return; it forwards `Display` to the
//! subsystem error it wraps.
//!
//! ```no_run
//! use logic_mesh::{Error, RegistryError};
//! # fn handle(err: Error) {
//! match err {
//!     Error::Registry(RegistryError::AmbiguousBlockName { libraries, .. }) => {
//!         // Retry qualified by one of `libraries`.
//!     }
//!     other => eprintln!("{other}"),
//! }
//! # }
//! ```
//!

pub mod engine;
pub mod external;
pub mod registry;
pub mod value;

pub use engine::{EngineError, LinkEnd};
pub use external::ExternalError;
pub use registry::RegistryError;
pub use value::ValueError;

pub(crate) use engine::parse_block_uuid;

use thiserror::Error;

/// Result alias used throughout the crate.
///
/// The error type defaults to [`Error`], while still allowing the second
/// type parameter to be named explicitly — either for a subsystem error
/// such as [`ValueError`], or for the internal actor protocols that
/// carry their own error payload.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The error every entry point of this crate reports, tagged by the
/// subsystem that produced it.
///
/// Match on the wrapped subsystem error to handle a specific failure;
/// `Display` and [`source`](std::error::Error::source) pass straight
/// through to it.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Block lookup, registration or evaluation failed.
    #[error(transparent)]
    Registry(#[from] RegistryError),

    /// Scheduling a block, wiring a link or reaching a block actor
    /// failed.
    #[error(transparent)]
    Engine(#[from] EngineError),

    /// Converting a value to the type a pin expects failed.
    #[error(transparent)]
    Value(#[from] ValueError),

    /// Resolving or running an external (host-provided) block failed.
    #[error(transparent)]
    External(#[from] ExternalError),
}

#[cfg(test)]
mod test {
    use super::{EngineError, Error, RegistryError};
    use assert_matches::assert_matches;

    /// The error type crosses task and thread boundaries (the engines
    /// hand it back out of spawned actor round-trips), so it has to stay
    /// `Send + Sync + 'static`.
    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<Error>();
    }

    /// Subsystem errors convert into the aggregate via `?`, and the
    /// aggregate keeps reporting the subsystem's own message.
    #[test]
    fn subsystem_errors_are_transparent() {
        let id = uuid::Uuid::new_v4();
        let err: Error = EngineError::BlockTaskGone { id }.into();

        assert_eq!(err.to_string(), format!("Block '{id}' actor task is gone"));
        assert_matches!(err, Error::Engine(EngineError::BlockTaskGone { .. }));
    }

    #[test]
    fn aggregate_keeps_the_subsystem_payload() {
        let err: Error = RegistryError::BlockNotFound {
            library: "core".to_string(),
            name: "Nope".to_string(),
        }
        .into();

        assert_matches!(
            err,
            Error::Registry(RegistryError::BlockNotFound { library, name })
                if library == "core" && name == "Nope"
        );
    }
}
