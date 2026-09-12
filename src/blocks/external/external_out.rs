// Copyright (c) 2022-2026, Radu Racariu.

//! External output block.

use libhaystack::val::Value;

use crate::base::error::ConnectorError;
use crate::base::{
    block::{Block, BlockProps, BlockState},
    connector::get_connector,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::external::support::{input_as_str, input_as_timeout_millis};
use crate::tokio_impl::block::drain_ready_inputs;
use crate::tokio_impl::sleep::sleep_millis;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Publishes the block's input value to an external system.
///
/// Resolves the [`Connector`](crate::base::connector::Connector)
/// registered under the name on the `connector` pin and publishes each
/// fresh `in` value to the address on the `address` pin. The published
/// value is echoed on `out` so downstream blocks can chain off a
/// successful publish; the block faults when the connector is missing,
/// the publish fails, or the publish does not settle within the
/// deadline on the `timeout` pin. The deadline is read as raw
/// milliseconds — the Number's unit is ignored — and must be finite
/// and greater than zero; fractions round up to the next millisecond,
/// and anything else (unset, non-Number, NaN, infinite, zero,
/// negative) falls back to 5000.
///
/// Only a fresh `in` value fires a publish: rewriting `connector`,
/// `address` or `timeout` alone re-binds without re-publishing the
/// cached value, and — like value flow everywhere in the engine —
/// re-emitting the value already cached on `in` does not fire. A value
/// that arrives before the `connector` and `address` pins resolve is
/// held and published once they do. Values reaching `in` through the
/// engine's pin-write path — a UI write, or a saved program's initial
/// value on load — count as fresh just like linked value flow.
///
/// # Delivery semantics
///
/// A timeout drops the in-flight publish future — by the connector
/// contract, dropping a publish aborts the operation where the
/// protocol allows — and the value is not re-published. Cancellation
/// from the block actor's mailbox takes the same drop path, but there
/// the value is kept and the publish is re-issued on the next cycle: a
/// publish cancelled mid-flight may already have reached the external
/// system and will be sent again, so subscribers should tolerate
/// duplicates (at-least-once semantics).
#[block]
#[derive(BlockProps, Debug)]
#[category = "external"]
pub struct ExternalOut {
    #[input(name = "in", kind = "Null")]
    pub input: InputImpl,
    #[input(kind = "Str")]
    pub connector: InputImpl,
    #[input(kind = "Str")]
    pub address: InputImpl,
    #[input(kind = "Number")]
    pub timeout: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
    /// The value awaiting a completed publish: set when a fresh `in`
    /// value arrives, kept across a cancelled `execute` so the publish
    /// is re-issued, and cleared on any completion — publish success,
    /// publish error, connector miss, or timeout.
    pending: Option<Value>,
    /// The last `in` cache value `execute` has taken note of — the
    /// freshness baseline. Persisted on the block rather than
    /// snapshotted per `execute` call because the cache can move while
    /// no `execute` is running: the engine's `WriteInput` mailbox
    /// command writes the cache directly (no watch traffic), and its
    /// own arrival is what cancels the in-flight `execute` — so a
    /// snapshot taken at the next entry would already contain the new
    /// value and the freshness compare could never fire.
    last_input: Option<Value>,
}

impl ExternalOut {
    /// Promotes a fresh `in` value to `pending`. Only a move of the
    /// `in` cache away from `last_input` is fresh — the cache moves on
    /// genuine value changes alone, so a config-only pin write cannot
    /// re-publish the cached value, and a retry of `pending` is never
    /// mistaken for fresh input. A fresh value replaces any held
    /// retry; `Null` fires nothing but still moves the baseline, so a
    /// later return to the previous value fires again.
    fn promote_fresh_input(&mut self) {
        if self.input.get_value() == self.last_input.as_ref() {
            return;
        }
        self.last_input = self.input.get_value().cloned();
        if let Some(value) = &self.last_input
            && !matches!(value, Value::Null)
        {
            self.pending = Some(value.clone());
        }
    }
}

impl Block for ExternalOut {
    async fn execute(&mut self) {
        // Promote before deciding whether to block on inputs: a value
        // the engine wrote into the `in` cache between `execute` calls
        // (a UI pin write, `load_program` seeding — both take the
        // `WriteInput` mailbox path) produces no watch traffic, so
        // waiting on inputs would park right past it.
        self.promote_fresh_input();

        // Drain without blocking only when the held value can be
        // published right away; otherwise block on inputs so the
        // actor does not spin. Config pins are still drained either
        // way, so rebinds are honored before every attempt.
        let can_publish_now = self.pending.is_some()
            && input_as_str(&self.connector).is_some()
            && input_as_str(&self.address).is_some();

        if can_publish_now {
            drain_ready_inputs(self);
        } else {
            self.read_inputs_until_ready().await;
        }
        self.promote_fresh_input();

        let Some(value) = self.pending.clone() else {
            return;
        };

        let (Some(connector), Some(address)) =
            (input_as_str(&self.connector), input_as_str(&self.address))
        else {
            return;
        };

        let timeout_millis = input_as_timeout_millis(&self.timeout);

        let Some(handle) = get_connector(&connector) else {
            self.pending = None;
            self.set_state(BlockState::fault(format!(
                "ExternalOut: no connector named '{connector}'"
            )));
            return;
        };

        // `biased` polls the publish arm first, so a publish settling
        // exactly on the deadline deterministically beats the timeout.
        // Losing the race drops the publish future, which aborts the
        // operation where the protocol allows — the same drop path a
        // cancelled `execute` takes, except a completed arm clears the
        // held value while a cancellation keeps it for the retry above.
        tokio::select! {
            biased;
            result = handle.publish(&address, value.clone()) => {
                self.pending = None;
                match result {
                    Ok(()) => self.out.set(value),
                    Err(err) => {
                        self.set_state(BlockState::fault(format!("ExternalOut: {err}")));
                    }
                }
            }
            _ = sleep_millis(timeout_millis) => {
                self.pending = None;
                let err = ConnectorError::Timeout {
                    address,
                    millis: timeout_millis,
                };
                self.set_state(BlockState::fault(format!("ExternalOut: {err}")));
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test {
    use std::sync::Arc;
    use std::time::Duration;

    use libhaystack::val::Value;

    use crate::base::block::test_utils::write_block_inputs;
    use crate::base::block::{Block, BlockProps};
    use crate::base::connector::{register_connector, unregister_connector};
    use crate::blocks::external::ExternalOut;
    use crate::blocks::external::support::mock::MockConnector;

    fn unique_name(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn write_all(block: &mut ExternalOut, connector: &str, value: Value) {
        write_block_inputs([
            (&mut block.input, value),
            (&mut block.connector, Value::make_str(connector)),
            (&mut block.address, Value::make_str("topic")),
        ])
        .await;
    }

    #[tokio::test]
    async fn publishes_and_echoes() {
        let name = unique_name("out-flow");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;

        let published = mock.published.lock().unwrap().clone();
        assert_eq!(published, vec![("topic".to_string(), 42.into())]);
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn missing_connector_faults() {
        let mut block = ExternalOut::new();
        write_all(&mut block, &unique_name("out-missing"), 42.into()).await;
        block.execute().await;
        assert!(block.state().is_fault());
    }

    #[tokio::test]
    async fn publish_error_faults() {
        let name = unique_name("out-pub-err");
        let mock = Arc::new(MockConnector {
            fail_publish: true,
            ..Default::default()
        });
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;

        assert!(block.state().is_fault());
        assert!(mock.published.lock().unwrap().is_empty());
        assert_eq!(block.out.value, Value::Null, "no echo on failure");

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn slow_publish_times_out() {
        let name = unique_name("out-timeout");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(200),
            ..Default::default()
        });
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(50))]).await;
        block.execute().await;

        assert!(block.state().is_fault());
        assert!(
            mock.published.lock().unwrap().is_empty(),
            "timed-out publish was aborted"
        );
        assert_eq!(block.out.value, Value::Null, "no echo on timeout");

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn timeout_pin_overrides_default() {
        let name = unique_name("out-override");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(1000))]).await;
        block.execute().await;

        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn zero_timeout_falls_back_to_default() {
        let name = unique_name("out-zero-timeout");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(0))]).await;
        block.execute().await;

        assert_eq!(
            block.out.value,
            42.into(),
            "default 5000ms deadline applied"
        );
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn negative_timeout_falls_back_to_default() {
        let name = unique_name("out-neg-timeout");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(-5))]).await;
        block.execute().await;

        assert_eq!(
            block.out.value,
            42.into(),
            "default 5000ms deadline applied"
        );
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn nan_timeout_falls_back_to_default() {
        // A non-Number can never reach the pin — type-checked pin
        // writes fault the block first — so the remaining invalid
        // shape a Number pin can carry is a non-finite value.
        let name = unique_name("out-nan-timeout");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(f64::NAN))]).await;
        block.execute().await;

        assert_eq!(
            block.out.value,
            42.into(),
            "default 5000ms deadline applied"
        );
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn config_rewrite_does_not_republish() {
        let name = unique_name("out-rebind");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;
        assert_eq!(mock.published.lock().unwrap().len(), 1);

        write_block_inputs([(&mut block.address, Value::make_str("other"))]).await;
        block.execute().await;

        assert_eq!(
            mock.published.lock().unwrap().len(),
            1,
            "a config-only pin write does not re-publish the cached input"
        );
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn cancelled_publish_is_retried() {
        let name = unique_name("out-retry");
        let mock = Arc::new(MockConnector {
            publish_delay_millis: Some(50),
            ..Default::default()
        });
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_all(&mut block, &name, 42.into()).await;

        // Drop `execute` mid-publish, as the block actor does when a
        // mailbox command arrives.
        {
            let fut = block.execute();
            tokio::pin!(fut);
            tokio::select! {
                _ = &mut fut => panic!("publish should still be in flight"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
        assert!(
            mock.published.lock().unwrap().is_empty(),
            "cancelled publish did not complete"
        );
        assert_eq!(block.out.value, Value::Null);

        block.execute().await;

        assert_eq!(
            mock.published.lock().unwrap().clone(),
            vec![("topic".to_string(), 42.into())],
            "cancelled publish is re-issued without fresh input"
        );
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    /// The engine's `WriteInput` mailbox command writes the `in` cache
    /// directly — no watch traffic — and the command's own arrival is
    /// what cancels the in-flight `execute`, so the write always lands
    /// between two `execute` calls. The next `execute` must still
    /// treat the value as fresh and publish it.
    #[tokio::test]
    async fn cache_write_between_executes_publishes() {
        use crate::base::Status;
        use crate::base::input::Input;

        let name = unique_name("out-cache-write");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = ExternalOut::new();
        write_block_inputs([
            (&mut block.connector, Value::make_str(&name)),
            (&mut block.address, Value::make_str("topic")),
        ])
        .await;

        // Mimic `BlockMailboxCmd::WriteInput`: a direct cache write on
        // the `in` pin, invisible to the watch machinery.
        block.input.set_value(42.into(), Status::Ok);

        tokio::time::timeout(Duration::from_secs(1), block.execute())
            .await
            .expect("execute publishes the cache-written value instead of parking");

        assert_eq!(
            mock.published.lock().unwrap().clone(),
            vec![("topic".to_string(), 42.into())],
            "a cache-written `in` value is published"
        );
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }
}
