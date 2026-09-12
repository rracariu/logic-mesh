// Copyright (c) 2022-2026, Radu Racariu.

//! External data integration function blocks.
//!
//! These blocks bridge the engine's pin graph to
//! [`Connector`](crate::base::connector::Connector) implementations:
//! [`ExternalIn`] streams values from a connector subscription into an
//! output pin, [`ExternalOut`] publishes input pin values through a
//! connector, and [`Request`] performs a request/response round trip
//! with a timeout. All resolve their connector by name from the
//! process-wide registry, so the same program works with any protocol
//! adapter that is registered under that name.

pub mod external_in;
pub mod external_out;
pub mod request;
pub(crate) mod support;

pub use external_in::ExternalIn;
pub use external_out::ExternalOut;
pub use request::Request;
