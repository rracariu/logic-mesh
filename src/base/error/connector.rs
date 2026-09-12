// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised by connectors — the protocol adapters that exchange
//! data between the engine and external systems.
//!

use thiserror::Error;

/// Failures of connector lookup and of the operations a
/// [`Connector`](crate::base::connector::Connector) performs.
///
/// The `Subscribe`, `Publish` and `Request` variants carry the address
/// the operation targeted plus a protocol-specific detail message, so
/// callers can tell *which* route failed without parsing the display
/// string.
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
    Subscribe {
        /// The address the subscription targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
    },

    /// Publishing a value to an address failed.
    #[error("Publish to '{address}' failed: {detail}")]
    Publish {
        /// The address the publish targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
    },

    /// A request/response operation failed.
    #[error("Request to '{address}' failed: {detail}")]
    Request {
        /// The address the request targeted.
        address: String,
        /// The protocol-specific failure message.
        detail: String,
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
    #[error("Transport error: {0}")]
    Transport(String),
}
