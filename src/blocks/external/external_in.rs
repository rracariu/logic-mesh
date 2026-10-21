// Copyright (c) 2022-2026, Radu Racariu.

//! External input block.

use std::time::Duration;

use futures::StreamExt;

use crate::base::{
    block::{Block, BlockProps, BlockState},
    connector::get_connector,
    input::input_reader::InputReader,
    output::Output,
};
use crate::blocks::external::support::{
    ActiveSubscription, Deadline, connector_fault, input_as_str, input_as_timeout_millis,
};
use crate::blocks::utils::get_sleep_dur;
use crate::tokio_impl::block::drain_ready_inputs;
use crate::tokio_impl::sleep::{monotonic_millis, sleep_millis};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Streams values from an external system into the block's output.
///
/// Binds to the [`Connector`](crate::base::connector::Connector)
/// registered under the name on the `connector` pin and subscribes to
/// the address on the `address` pin. Every value the subscription
/// yields is set on `out`. Changing either pin drops the current
/// subscription — which ends it — and opens a new one; the block
/// faults when the connector is missing, the subscribe fails or does
/// not settle within the deadline on the `timeout` pin, the stream
/// yields an error, or the stream ends. A stream ended by a connector
/// stop, and a subscribe that failed or timed out, are retried
/// automatically on a later cycle (throttled by the polling interval),
/// so the block recovers by itself after the connector restarts.
///
/// The deadline is read like ExternalOut's: as raw milliseconds — the
/// Number's unit is ignored — finite and greater than zero, fractions
/// rounding up, and anything else falling back to 5000. It is counted
/// from when a binding's subscribe was first issued and keeps running
/// when an engine request addressed to the block cancels the cycle —
/// the subscribe is then re-issued with what is left of it — so steady
/// request traffic cannot keep a slow subscribe from timing out.
/// Rewriting `timeout` while a subscribe is owed applies to that
/// subscribe without restarting its clock — a program load, for one,
/// writes `timeout` after `connector` and `address` — and rewriting it
/// alone never re-subscribes.
#[block]
#[derive(BlockProps, Debug)]
#[category = "external"]
pub struct ExternalIn {
    #[input(kind = "Str")]
    pub connector: InputImpl,
    #[input(kind = "Str")]
    pub address: InputImpl,
    #[input(kind = "Number")]
    pub timeout: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
    subscription: Option<ActiveSubscription>,
    /// The subscribe owed while it has not settled: kept across a
    /// cancelled `execute`, with the time it was first issued, until
    /// the subscribe completes, fails or times out, or the binding
    /// changes.
    binding: Option<PendingBinding>,
}

/// A subscribe [`ExternalIn`] has issued and not seen settle.
#[derive(Debug)]
struct PendingBinding {
    /// Name the connector was resolved under.
    connector: String,
    /// Address the subscribe targets.
    address: String,
    /// When the subscribe was first issued, on the
    /// [`monotonic_millis`] clock.
    issued_at: u64,
}

impl Block for ExternalIn {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();

        if self.subscription.is_some() || self.binding.is_some() {
            // Already streaming, or owing a subscribe a cancelled cycle
            // cut short — pick up pin rewrites without stalling behind
            // an input wait.
            drain_ready_inputs(self);
        } else {
            self.wait_on_inputs(Duration::from_millis(poll)).await;
        }

        let (Some(connector), Some(address)) =
            (input_as_str(&self.connector), input_as_str(&self.address))
        else {
            self.subscription = None;
            self.binding = None;
            return;
        };

        let needs_bind = self
            .subscription
            .as_ref()
            .is_none_or(|sub| sub.connector != connector || sub.address != address);

        if needs_bind {
            // Drop any previous subscription before opening the new one;
            // dropping the stream is the unsubscribe.
            self.subscription = None;

            let Some(handle) = get_connector(&connector) else {
                self.binding = None;
                self.set_state(BlockState::fault(format!(
                    "ExternalIn: no connector named '{connector}'"
                )));
                return;
            };

            // The same binding keeps counting from its first
            // subscribe; a new one starts its own clock. The timeout is
            // read afresh either way, so one written since applies.
            let issued_at = match &self.binding {
                Some(binding) if binding.connector == connector && binding.address == address => {
                    binding.issued_at
                }
                _ => monotonic_millis(),
            };
            let deadline = Deadline::since(issued_at, input_as_timeout_millis(&self.timeout));
            if deadline.has_passed() {
                self.binding = None;
                let err = deadline.timeout_error(address);
                self.set_state(connector_fault("ExternalIn", &err));
                return;
            }
            self.binding = Some(PendingBinding {
                connector: connector.clone(),
                address: address.clone(),
                issued_at,
            });

            // `biased`: a subscribe settling exactly on the deadline
            // beats the timeout, as for ExternalOut's publish.
            let subscribed = tokio::select! {
                biased;
                subscribed = handle.subscribe(&address) => subscribed,
                () = deadline.passed() => Err(deadline.timeout_error(address.clone())),
            };
            self.binding = None;
            match subscribed {
                Ok(stream) => {
                    self.subscription = Some(ActiveSubscription {
                        connector,
                        address,
                        stream,
                    });
                }
                Err(err) => {
                    self.set_state(connector_fault("ExternalIn", &err));
                    return;
                }
            }
        }

        let Some(sub) = self.subscription.as_mut() else {
            return;
        };

        // Race the stream against the polling interval so pin rewrites
        // are noticed even while the external source is quiet. The
        // stream stays owned by `self`, so cancelling this select — or
        // the whole `execute` from the block actor — keeps the
        // subscription alive; channel-backed streams lose no values on
        // a cancelled `next()`.
        tokio::select! {
            item = sub.stream.next() => match item {
                Some(Ok(value)) => self.out.set(value),
                Some(Err(err)) => {
                    self.set_state(connector_fault("ExternalIn", &err));
                }
                None => {
                    // Stream ended — fault for visibility and drop the
                    // binding so the next cycle re-subscribes (the
                    // polling-interval wait throttles the retries).
                    self.subscription = None;
                    self.set_state(BlockState::fault(
                        "ExternalIn: subscription ended".to_string(),
                    ));
                }
            },
            _ = sleep_millis(poll) => {}
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use std::sync::Arc;

    use libhaystack::val::Value;

    use crate::base::block::test_utils::write_block_inputs;
    use crate::base::block::{Block, BlockProps};
    use crate::base::connector::{register_connector, unregister_connector};
    use crate::base::error::ConnectorError;
    use crate::blocks::external::ExternalIn;
    use crate::blocks::external::support::mock::{MOCK_CAUSE, MockConnector};

    fn unique_name(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn bind(block: &mut ExternalIn, connector: &str, address: &str) {
        write_block_inputs([
            (&mut block.connector, Value::make_str(connector)),
            (&mut block.address, Value::make_str(address)),
        ])
        .await;
        block.execute().await;
    }

    #[tokio::test]
    async fn streams_values_to_out() {
        let name = unique_name("in-flow");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalIn::new();
        bind(&mut block, &name, "topic").await;
        assert!(mock.has_subscription("topic"), "first cycle subscribes");

        mock.feed("topic", Ok(42.into()));
        block.execute().await;
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn missing_connector_faults() {
        let mut block = ExternalIn::new();
        bind(&mut block, &unique_name("in-missing"), "topic").await;
        assert!(block.state().is_fault());
    }

    /// Regression: a subscribe had no deadline, so one that never
    /// settled parked the block for good.
    #[tokio::test]
    async fn slow_subscribe_times_out() {
        let name = unique_name("in-timeout");
        let mock = Arc::new(MockConnector {
            subscribe_delay_millis: Some(10_000),
            ..Default::default()
        });
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalIn::new();
        write_block_inputs([(&mut block.timeout, Value::from(50))]).await;
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            bind(&mut block, &name, "topic"),
        )
        .await
        .expect("the subscribe times out instead of parking");

        assert!(block.state().is_fault());
        assert!(!mock.has_subscription("topic"), "no subscription opened");

        unregister_connector(&name);
    }

    /// A subscribe cut short by a cancelled cycle keeps its deadline:
    /// re-issuing it does not restart the clock.
    #[tokio::test]
    async fn cancelled_subscribe_keeps_its_deadline() {
        let name = unique_name("in-cancelled-timeout");
        let mock = Arc::new(MockConnector {
            subscribe_delay_millis: Some(10_000),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalIn::new();
        write_block_inputs([
            (&mut block.timeout, Value::from(100)),
            (&mut block.connector, Value::make_str(&name)),
            (&mut block.address, Value::make_str("topic")),
        ])
        .await;

        // Drop `execute` every 20 ms, as the block actor does when
        // mailbox commands keep arriving.
        let started = std::time::Instant::now();
        while !block.state().is_fault() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(2),
                "the subscribe never timed out"
            );
            let _ =
                tokio::time::timeout(std::time::Duration::from_millis(20), block.execute()).await;
        }

        unregister_connector(&name);
    }

    /// The fault reason carries the connector's underlying error too
    /// (regression: it printed only the error's own message).
    #[tokio::test]
    async fn subscribe_error_faults() {
        let name = unique_name("in-sub-err");
        let mock = Arc::new(MockConnector {
            fail_subscribe: true,
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalIn::new();
        bind(&mut block, &name, "topic").await;
        assert_eq!(
            block.state().fault_reason(),
            Some(
                format!(
                    "ExternalIn: Subscribe to 'topic' failed: mock subscribe failure: {MOCK_CAUSE}"
                )
                .as_str()
            )
        );

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn stream_error_faults() {
        let name = unique_name("in-stream-err");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalIn::new();
        bind(&mut block, &name, "topic").await;

        mock.feed(
            "topic",
            Err(ConnectorError::transport("boom")
                .with_source(std::io::Error::other("connection reset"))),
        );
        block.execute().await;
        assert_eq!(
            block.state().fault_reason(),
            Some("ExternalIn: Transport error: boom: connection reset")
        );

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn rebinds_on_address_change() {
        let name = unique_name("in-rebind");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalIn::new();
        bind(&mut block, &name, "first").await;
        assert!(mock.has_subscription("first"));

        write_block_inputs([(&mut block.address, Value::make_str("second"))]).await;
        block.execute().await;

        assert!(mock.has_subscription("second"), "new address subscribed");
        let old_closed = mock.senders.lock().unwrap()["first"].is_closed();
        assert!(old_closed, "dropping the old stream ends the subscription");

        unregister_connector(&name);
    }
}
