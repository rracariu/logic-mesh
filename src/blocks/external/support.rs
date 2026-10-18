// Copyright (c) 2022-2026, Radu Racariu.

//! Shared plumbing for the external integration blocks.

use libhaystack::val::Value;

use crate::base::connector::ValueStream;
use crate::base::error::ConnectorError;
use crate::base::input::InputProps;
use crate::blocks::InputImpl;
use crate::tokio_impl::sleep::{monotonic_millis, sleep_millis};

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
        Some(Value::Str(s)) if !s.is_empty() => Some(s.value().to_owned()),
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

/// The point in time a connector operation must settle by: the time it
/// was first issued plus the block's timeout.
///
/// The block actor drops a block's in-flight `execute` whenever a
/// mailbox command arrives, and the next cycle re-issues the operation.
/// Keeping the time of the first issue on the block across those cycles
/// is what lets steady command traffic re-issue an operation without
/// restarting its clock. Each re-issue builds its deadline anew from
/// that time and the `timeout` pin's current value, so a timeout
/// written while the operation is owed applies to it — a program load,
/// for one, writes `timeout` after the pins that start the operation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Deadline {
    /// The timeout the deadline was set with, in milliseconds.
    millis: u64,
    /// When it runs out, on the [`monotonic_millis`] clock.
    at: u64,
}

impl Deadline {
    /// A deadline `millis` milliseconds after `issued_at`, a reading of
    /// the [`monotonic_millis`] clock.
    pub(crate) fn since(issued_at: u64, millis: u64) -> Self {
        Self {
            millis,
            at: issued_at.saturating_add(millis),
        }
    }

    /// The time left, in milliseconds; zero once the deadline passed.
    fn remaining_millis(&self) -> u64 {
        self.at.saturating_sub(monotonic_millis())
    }

    /// Whether the deadline has passed.
    pub(crate) fn has_passed(&self) -> bool {
        self.remaining_millis() == 0
    }

    /// Resolves once the deadline has passed.
    pub(crate) async fn passed(&self) {
        sleep_millis(self.remaining_millis()).await;
    }

    /// The error reporting an operation on `address` that missed this
    /// deadline.
    pub(crate) fn timeout_error(&self, address: String) -> ConnectorError {
        ConnectorError::Timeout {
            address,
            millis: self.millis,
        }
    }
}

/// A connector operation an external block owes: the value to send and,
/// from the first time the operation is issued, when that was. Kept on
/// the block until the operation completes, so a cancelled `execute`
/// re-issues it with the time it has left.
#[derive(Debug)]
pub(crate) struct PendingOp {
    /// The value the operation sends.
    pub(crate) value: Value,
    /// When the operation was first issued, on the [`monotonic_millis`]
    /// clock.
    issued_at: Option<u64>,
}

impl PendingOp {
    /// An operation sending `value`, not issued yet.
    pub(crate) fn new(value: Value) -> Self {
        Self {
            value,
            issued_at: None,
        }
    }

    /// The operation's [`Deadline`]: `millis` milliseconds after it was
    /// first issued — now, if this is the first time.
    pub(crate) fn deadline(&mut self, millis: u64) -> Deadline {
        let issued_at = *self.issued_at.get_or_insert_with(monotonic_millis);
        Deadline::since(issued_at, millis)
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
        /// When set, `subscribe` sleeps this many milliseconds before
        /// opening the subscription.
        pub(crate) subscribe_delay_millis: Option<u64>,
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
                if let Some(millis) = self.subscribe_delay_millis {
                    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
                }
                if self.fail_subscribe {
                    return Err(ConnectorError::subscribe(address, "mock subscribe failure"));
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
                    return Err(ConnectorError::publish(address, "mock publish failure"));
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
                    return Err(ConnectorError::request(address, "mock request failure"));
                }
                Ok(value)
            })
        }
    }
}
