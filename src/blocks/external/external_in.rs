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
use crate::blocks::external::support::{ActiveSubscription, input_as_str};
use crate::blocks::utils::get_sleep_dur;
use crate::tokio_impl::block::drain_ready_inputs;
use crate::tokio_impl::sleep::sleep_millis;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Streams values from an external system into the block's output.
///
/// Binds to the [`Connector`](crate::base::connector::Connector)
/// registered under the name on the `connector` pin and subscribes to
/// the address on the `address` pin. Every value the subscription
/// yields is set on `out`. Changing either pin drops the current
/// subscription — which ends it — and opens a new one; the block
/// faults when the connector is missing, the subscribe fails, the
/// stream yields an error, or the stream ends. A stream ended by a
/// connector stop is re-subscribed automatically on the next cycle
/// (throttled by the polling interval), so the block recovers by
/// itself after the connector restarts.
#[block]
#[derive(BlockProps, Debug)]
#[category = "external"]
pub struct ExternalIn {
    #[input(kind = "Str")]
    pub connector: InputImpl,
    #[input(kind = "Str")]
    pub address: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
    subscription: Option<ActiveSubscription>,
}

impl Block for ExternalIn {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();

        if self.subscription.is_some() {
            // Already streaming — pick up pin rewrites without stalling
            // the stream behind an input wait.
            drain_ready_inputs(self);
        } else {
            self.wait_on_inputs(Duration::from_millis(poll)).await;
        }

        let (Some(connector), Some(address)) =
            (input_as_str(&self.connector), input_as_str(&self.address))
        else {
            self.subscription = None;
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
                self.set_state(BlockState::fault(format!(
                    "ExternalIn: no connector named '{connector}'"
                )));
                return;
            };

            match handle.subscribe(&address).await {
                Ok(stream) => {
                    self.subscription = Some(ActiveSubscription {
                        connector,
                        address,
                        stream,
                    });
                }
                Err(err) => {
                    self.set_state(BlockState::fault(format!("ExternalIn: {err}")));
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
                    self.set_state(BlockState::fault(format!("ExternalIn: {err}")));
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
    use crate::blocks::external::support::mock::MockConnector;

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
        assert!(block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn stream_error_faults() {
        let name = unique_name("in-stream-err");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalIn::new();
        bind(&mut block, &name, "topic").await;

        mock.feed("topic", Err(ConnectorError::Transport("boom".to_string())));
        block.execute().await;
        assert!(block.state().is_fault());

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
