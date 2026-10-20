// Copyright (c) 2022-2026, Radu Racariu.

//!
//! The error an engine answers a failed request with.
//!

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{
    ConnectError, ConnectorError, EngineError, Error, ErrorChain, ErrorCode, ExternalError,
    RegistryError, ValueError,
};

/// Why an engine request failed, as an
/// [`EngineResponse`](crate::base::engine::messages::EngineResponse)
/// carries it.
///
/// A response crosses tasks, threads and — through a custom transport or
/// the JavaScript bindings — process boundaries, which the crate's own
/// errors cannot all cross: they are not all [`Clone`], and some wrap
/// errors that cannot be serialized. A request error is both, and keeps
/// what a caller across such a boundary needs:
///
/// - [`code`](Self::code), the stable [`ErrorCode`] of the variant that
///   failed, to branch on;
/// - [`message`](Self::message), the failure's message followed by every
///   error in its [`source`](std::error::Error::source) chain, to show.
///
/// Within the process, [`error`](Self::error) hands back the typed
/// [`enum@Error`] the engine reported, with its data, to match on. It is
/// not serialized, so a request error read back from a serialized form
/// has none.
///
/// Two request errors are equal when their codes and messages are.
///
/// # Examples
///
/// ```
/// use logic_mesh::{EngineError, Error, ErrorCode, RequestError, Uuid};
///
/// let id = Uuid::nil();
/// let err = RequestError::from(EngineError::BlockInstanceNotFound { id });
///
/// assert_eq!(err.code, ErrorCode::BlockInstanceNotFound);
/// assert_eq!(err.message, format!("Block instance '{id}' not found"));
/// assert!(matches!(
///     err.error(),
///     Some(Error::Engine(EngineError::BlockInstanceNotFound { .. }))
/// ));
///
/// let json = serde_json::to_string(&err)?;
/// assert_eq!(
///     json,
///     format!(r#"{{"code":"BlockInstanceNotFound","message":"Block instance '{id}' not found"}}"#)
/// );
/// let read: RequestError = serde_json::from_str(&json)?;
/// assert_eq!(read, err);
/// assert!(read.error().is_none());
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RequestError {
    /// The code of the failure.
    pub code: ErrorCode,
    /// The failure's message, followed by its sources' messages, joined
    /// by `": "`.
    pub message: String,
    /// The typed error, while the request error stays in the process
    /// that produced it.
    #[serde(skip)]
    error: Option<Arc<Error>>,
}

impl RequestError {
    /// A request error with `code` and `message` and no typed error — a
    /// failure a transport reports itself, or one read back from a
    /// serialized form.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            error: None,
        }
    }

    /// The typed error the engine reported, if this request error was
    /// built from one in this process.
    pub fn error(&self) -> Option<&Error> {
        self.error.as_deref()
    }
}

impl PartialEq for RequestError {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code && self.message == other.message
    }
}

impl Eq for RequestError {}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

/// The message already renders the sources, so a request error reports
/// none of its own: a reporter walking the chain prints each once.
impl std::error::Error for RequestError {}

impl From<Error> for RequestError {
    fn from(error: Error) -> Self {
        Self {
            code: error.code(),
            message: ErrorChain(&error).to_string(),
            error: Some(Arc::new(error)),
        }
    }
}

/// Implements `From<$error> for RequestError` through [`enum@Error`].
macro_rules! from_subsystem_error {
    ($($error:ty),*) => {
        $(
            impl From<$error> for RequestError {
                fn from(error: $error) -> Self {
                    Error::from(error).into()
                }
            }
        )*
    };
}

from_subsystem_error!(
    RegistryError,
    EngineError,
    ConnectError,
    ValueError,
    ExternalError,
    ConnectorError
);

#[cfg(test)]
mod test {
    use super::RequestError;
    use crate::base::error::{ConnectorError, EngineError, Error, ErrorCode};
    use assert_matches::assert_matches;

    /// The message walks the source chain, so a wrapped cause reaches a
    /// caller that only sees the message.
    #[test]
    fn the_message_includes_every_source() {
        let err = RequestError::from(EngineError::ConnectorStartFailed {
            name: "mqtt".to_string(),
            source: ConnectorError::transport("refused")
                .with_source(std::io::Error::other("port in use")),
        });

        assert_eq!(err.code, ErrorCode::ConnectorStartFailed);
        assert_eq!(
            err.message,
            "Connector 'mqtt' failed to start: Transport error: refused: port in use"
        );
        assert_eq!(err.to_string(), err.message);
    }

    /// A clone shares the typed error.
    #[test]
    fn a_clone_keeps_the_typed_error() {
        let err = RequestError::from(EngineError::EngineGone).clone();

        assert_matches!(err.error(), Some(Error::Engine(EngineError::EngineGone)));
    }

    /// Request errors cross threads inside responses.
    #[test]
    fn request_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<RequestError>();
    }
}
