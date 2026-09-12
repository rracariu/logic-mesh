// Copyright (c) 2022-2026, Radu Racariu.

//! Shared plumbing for the external integration blocks.

use libhaystack::val::Value;

use crate::base::connector::ValueStream;
use crate::base::input::InputProps;
use crate::blocks::InputImpl;

/// A live connector subscription held by an external block across
/// execution cycles: the binding identity plus the value stream it
/// produced. Dropping it drops the stream, which ends the subscription.
pub(crate) struct ActiveSubscription {
    /// Name the connector was resolved under.
    pub(crate) connector: String,
    /// Address the subscription targets.
    pub(crate) address: String,
    /// The values the subscription yields.
    pub(crate) stream: ValueStream,
}

impl std::fmt::Debug for ActiveSubscription {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("ActiveSubscription")
            .field("connector", &self.connector)
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

/// Reads a `Str` input as a non-empty string, or [`None`] if the pin has
/// no value, a non-string value, or an empty string.
pub(crate) fn input_as_str(input: &InputImpl) -> Option<String> {
    match input.get_value() {
        Some(Value::Str(s)) if !s.value.is_empty() => Some(s.value.clone()),
        _ => None,
    }
}

/// The deadline used when a block's `timeout` pin is unset or invalid,
/// in milliseconds.
pub(crate) const DEFAULT_TIMEOUT_MILLIS: u64 = 5000;

/// Reads a `Number` input as a millisecond deadline. The value is read
/// as raw milliseconds — the Number's unit is ignored — and must be
/// finite and greater than zero; fractions round up to the next
/// millisecond, and anything else (unset, non-Number, NaN, infinite,
/// zero, negative) falls back to [`DEFAULT_TIMEOUT_MILLIS`].
pub(crate) fn input_as_timeout_millis(input: &InputImpl) -> u64 {
    match input.get_value() {
        Some(Value::Number(num)) if num.value.is_finite() && num.value > 0.0 => {
            num.value.ceil() as u64
        }
        _ => DEFAULT_TIMEOUT_MILLIS,
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod mock {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use futures::channel::mpsc;
    use libhaystack::val::Value;

    use crate::base::connector::{Connector, ConnectorFuture, ValueStream};
    use crate::base::error::ConnectorError;

    /// A scriptable [`Connector`] for tests: subscriptions are backed by
    /// channels the test feeds, publishes are recorded, and each
    /// operation can be made to fail.
    #[derive(Default)]
    pub(crate) struct MockConnector {
        /// Feed side of every open subscription, keyed by address.
        pub(crate) senders:
            Mutex<HashMap<String, mpsc::UnboundedSender<Result<Value, ConnectorError>>>>,
        /// Every `(address, value)` pair published through the connector.
        pub(crate) published: Mutex<Vec<(String, Value)>>,
        /// Every `(address, value)` pair requested through the connector.
        pub(crate) requests: Mutex<Vec<(String, Value)>>,
        /// When set, `subscribe` fails with [`ConnectorError::Subscribe`].
        pub(crate) fail_subscribe: bool,
        /// When set, `publish` fails with [`ConnectorError::Publish`].
        pub(crate) fail_publish: bool,
        /// When set, `publish` sleeps this many milliseconds before
        /// recording and acknowledging the value.
        pub(crate) publish_delay_millis: Option<u64>,
        /// When set, `request` fails with [`ConnectorError::Request`].
        pub(crate) fail_request: bool,
        /// When set, `request` sleeps this many milliseconds before
        /// responding.
        pub(crate) request_delay_millis: Option<u64>,
    }

    impl MockConnector {
        /// True once a subscription is open for `address`.
        pub(crate) fn has_subscription(&self, address: &str) -> bool {
            self.senders.lock().unwrap().contains_key(address)
        }

        /// Pushes `item` into the subscription open for `address`.
        ///
        /// # Panics
        ///
        /// Panics if no subscription is open for `address`.
        pub(crate) fn feed(&self, address: &str, item: Result<Value, ConnectorError>) {
            self.senders
                .lock()
                .unwrap()
                .get(address)
                .expect("subscription is open")
                .unbounded_send(item)
                .expect("subscriber still listening");
        }
    }

    impl Connector for MockConnector {
        fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream> {
            let address = address.to_string();
            Box::pin(async move {
                if self.fail_subscribe {
                    return Err(ConnectorError::Subscribe {
                        address,
                        detail: "mock subscribe failure".to_string(),
                    });
                }
                let (tx, rx) = mpsc::unbounded();
                self.senders.lock().unwrap().insert(address, tx);
                Ok(Box::pin(rx) as ValueStream)
            })
        }

        fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()> {
            let address = address.to_string();
            Box::pin(async move {
                if let Some(millis) = self.publish_delay_millis {
                    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
                }
                if self.fail_publish {
                    return Err(ConnectorError::Publish {
                        address,
                        detail: "mock publish failure".to_string(),
                    });
                }
                self.published.lock().unwrap().push((address, value));
                Ok(())
            })
        }

        fn request(&self, address: &str, value: Value) -> ConnectorFuture<'_, Value> {
            let address = address.to_string();
            Box::pin(async move {
                self.requests
                    .lock()
                    .unwrap()
                    .push((address.clone(), value.clone()));
                if let Some(millis) = self.request_delay_millis {
                    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
                }
                if self.fail_request {
                    return Err(ConnectorError::Request {
                        address,
                        detail: "mock request failure".to_string(),
                    });
                }
                Ok(value)
            })
        }
    }
}
