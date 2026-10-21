// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised by connectors — the protocol adapters that exchange
//! data between the engine and external systems.
//!

use thiserror::Error;

/// The underlying error a connector failure can carry, such as the
/// protocol client's own error.
pub type ConnectorErrorSource = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Failures of connector lookup and of the operations a
/// [`Connector`](crate::base::connector::Connector) performs.
///
/// The `Subscribe`, `Publish` and `Request` variants carry the address
/// the operation targeted plus a protocol-specific detail message, so
/// callers can tell *which* route failed without parsing the display
/// string.
///
/// The variants a connector reports — `Subscribe`, `Publish`, `Request`
/// and `Transport` — are `#[non_exhaustive]`: build them with
/// [`subscribe`](Self::subscribe), [`publish`](Self::publish),
/// [`request`](Self::request) and [`transport`](Self::transport), and
/// attach the protocol client's error with
/// [`with_source`](Self::with_source). It is exposed through
/// [`source`](std::error::Error::source) and left out of the message,
/// so a reporter that walks the error chain prints it once. The
/// crate's own reports — the fault reasons of the external blocks and
/// the error of a connector that fails to start — walk the chain, so
/// they include it.
///
/// # Examples
///
/// ```
/// use std::error::Error as _;
/// use logic_mesh::ConnectorError;
///
/// let io = std::io::Error::other("connection reset");
/// let err = ConnectorError::publish("sensors/t1", "broker refused").with_source(io);
///
/// assert_eq!(err.to_string(), "Publish to 'sensors/t1' failed: broker refused");
/// assert_eq!(err.source().map(|s| s.to_string()).as_deref(), Some("connection reset"));
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectorError {
    /// No connector is registered under the requested name.
    #[error("Connector '{name}' is not registered")]
    NotFound {
        /// The connector name that was looked up.
        name: String,
    },

    /// A connector is already registered under this name.
    #[error("Connector '{name}' is already registered")]
    AlreadyRegistered {
        /// The name the registration collided on.
        name: String,
    },

    /// Subscribing to an address failed.
    #[error("Subscribe to '{address}' failed: {detail}")]
    #[non_exhaustive]
    Subscribe {
        /// The address the subscription targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
        /// The underlying error, if the connector has one.
        #[source]
        source: Option<ConnectorErrorSource>,
    },

    /// Publishing a value to an address failed.
    #[error("Publish to '{address}' failed: {detail}")]
    #[non_exhaustive]
    Publish {
        /// The address the publish targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
        /// The underlying error, if the connector has one.
        #[source]
        source: Option<ConnectorErrorSource>,
    },

    /// A request/response operation failed.
    #[error("Request to '{address}' failed: {detail}")]
    #[non_exhaustive]
    Request {
        /// The address the request targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
        /// The underlying error, if the connector has one.
        #[source]
        source: Option<ConnectorErrorSource>,
    },

    /// An operation did not complete within its deadline.
    #[error("Operation on '{address}' timed out after {millis}ms")]
    Timeout {
        /// The address the timed-out operation targeted.
        address: String,
        /// The deadline that elapsed, in milliseconds.
        millis: u64,
    },

    /// The connector's underlying transport failed outside of a
    /// specific operation — e.g. a dropped connection.
    #[error("Transport error: {detail}")]
    #[non_exhaustive]
    Transport {
        /// The protocol-specific failure message.
        detail: String,
        /// The underlying error, if the connector has one.
        #[source]
        source: Option<ConnectorErrorSource>,
    },
}

impl ConnectorError {
    /// A [`Subscribe`](Self::Subscribe) failure for `address`, with no
    /// source.
    pub fn subscribe(address: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Subscribe {
            address: address.into(),
            detail: detail.into(),
            source: None,
        }
    }

    /// A [`Publish`](Self::Publish) failure for `address`, with no
    /// source.
    pub fn publish(address: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Publish {
            address: address.into(),
            detail: detail.into(),
            source: None,
        }
    }

    /// A [`Request`](Self::Request) failure for `address`, with no
    /// source.
    pub fn request(address: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Request {
            address: address.into(),
            detail: detail.into(),
            source: None,
        }
    }

    /// A [`Transport`](Self::Transport) failure, with no source.
    pub fn transport(detail: impl Into<String>) -> Self {
        Self::Transport {
            detail: detail.into(),
            source: None,
        }
    }

    /// Sets the underlying error of a `Subscribe`, `Publish`, `Request`
    /// or `Transport` failure, replacing any it had.
    ///
    /// The other variants describe failures of the connector registry
    /// and the engine, which have no underlying error: they are
    /// returned unchanged.
    pub fn with_source(mut self, error: impl Into<ConnectorErrorSource>) -> Self {
        match &mut self {
            Self::Subscribe { source, .. }
            | Self::Publish { source, .. }
            | Self::Request { source, .. }
            | Self::Transport { source, .. } => *source = Some(error.into()),
            Self::NotFound { .. } | Self::AlreadyRegistered { .. } | Self::Timeout { .. } => {}
        }
        self
    }
}

#[cfg(test)]
mod test {
    use std::error::Error as _;

    use super::ConnectorError;

    #[test]
    fn constructors_have_no_source() {
        for err in [
            ConnectorError::subscribe("a", "x"),
            ConnectorError::publish("a", "x"),
            ConnectorError::request("a", "x"),
            ConnectorError::transport("x"),
        ] {
            assert!(err.source().is_none(), "{err:?}");
        }
    }

    /// The source is reported through `source()` and not repeated in
    /// the message.
    #[test]
    fn source_is_exposed_and_not_repeated() {
        let err = ConnectorError::transport("link down")
            .with_source(std::io::Error::other("connection reset"));

        assert_eq!(err.to_string(), "Transport error: link down");
        assert_eq!(
            err.source().map(|s| s.to_string()).as_deref(),
            Some("connection reset")
        );

        let err = ConnectorError::request("svc", "bad reply").with_source("parse error");
        assert_eq!(err.to_string(), "Request to 'svc' failed: bad reply");
        assert_eq!(
            err.source().map(|s| s.to_string()).as_deref(),
            Some("parse error")
        );
    }

    /// The aggregate error forwards the connector's source.
    #[test]
    fn aggregate_error_forwards_the_source() {
        let err: crate::Error = ConnectorError::subscribe("t", "denied")
            .with_source(std::io::Error::other("acl"))
            .into();

        assert_eq!(err.source().map(|s| s.to_string()).as_deref(), Some("acl"));
    }

    #[test]
    fn registry_failures_take_no_source() {
        let err = ConnectorError::NotFound {
            name: "mqtt".to_string(),
        }
        .with_source(std::io::Error::other("ignored"));

        assert!(err.source().is_none());
    }
}
