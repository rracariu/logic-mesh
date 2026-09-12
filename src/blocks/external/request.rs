// Copyright (c) 2022-2026, Radu Racariu.

//! External request block.

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

/// Performs a request/response round trip through an external system.
///
/// Resolves the [`Connector`](crate::base::connector::Connector)
/// registered under the name on the `connector` pin and sends each
/// fresh `in` value as a request to the address on the `address` pin.
/// The response is set on `out`; the block faults when the connector
/// is missing, the request fails, or no response arrives within the
/// deadline on the `timeout` pin. The deadline is read as raw
/// milliseconds — the Number's unit is ignored — and must be finite
/// and greater than zero; fractions round up to the next millisecond,
/// and anything else (unset, non-Number, NaN, infinite, zero,
/// negative) falls back to 5000.
///
/// Only a fresh `in` value fires a request: rewriting `connector`,
/// `address` or `timeout` alone re-binds without re-sending, and —
/// like value flow everywhere in the engine — re-emitting the value
/// already cached on `in` does not fire. A value that arrives before
/// the `connector` and `address` pins resolve is held and sent once
/// they do. Values reaching `in` through the engine's pin-write path —
/// a UI write, or a saved program's initial value on load — count as
/// fresh just like linked value flow.
///
/// # Delivery semantics
///
/// A timeout drops the in-flight request future — by the connector
/// contract, dropping a request aborts the operation where the
/// protocol allows — and the value is not re-sent. Cancellation from
/// the block actor's mailbox takes the same drop path, but there the
/// value is kept and the request is re-issued on the next cycle: a
/// request cancelled mid-flight may already have reached the server
/// and will be sent again, so request handlers should be idempotent
/// or deduplicate by key (at-least-once semantics).
#[block]
#[derive(BlockProps, Debug)]
#[category = "external"]
pub struct Request {
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
    /// The value awaiting a completed request: set when a fresh `in`
    /// value arrives, kept across a cancelled `execute` so the request
    /// is re-issued, and cleared on any completion — response, request
    /// error, connector miss, or timeout.
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

impl Request {
    /// Promotes a fresh `in` value to `pending`. Only a move of the
    /// `in` cache away from `last_input` is fresh — the cache moves on
    /// genuine value changes alone, so a config-only pin write cannot
    /// re-fire the request that produced it, and a retry of `pending`
    /// is never mistaken for fresh input. A fresh value replaces any
    /// held retry; `Null` fires nothing but still moves the baseline,
    /// so a later return to the previous value fires again.
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

impl Block for Request {
    async fn execute(&mut self) {
        // Promote before deciding whether to block on inputs: a value
        // the engine wrote into the `in` cache between `execute` calls
        // (a UI pin write, `load_program` seeding — both take the
        // `WriteInput` mailbox path) produces no watch traffic, so
        // waiting on inputs would park right past it.
        self.promote_fresh_input();

        // Drain without blocking only when the held request can be
        // issued right away; otherwise block on inputs so the actor
        // does not spin. Config pins are still drained either way, so
        // rebinds are honored before every attempt.
        let can_send_now = self.pending.is_some()
            && input_as_str(&self.connector).is_some()
            && input_as_str(&self.address).is_some();

        if can_send_now {
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
                "Request: no connector named '{connector}'"
            )));
            return;
        };

        // `biased` polls the response arm first, so a response landing
        // exactly on the deadline deterministically beats the timeout.
        // Losing the race drops the request future, which aborts the
        // operation where the protocol allows — the same drop path a
        // cancelled `execute` takes, except a completed arm clears the
        // held value while a cancellation keeps it for the retry above.
        tokio::select! {
            biased;
            response = handle.request(&address, value) => {
                self.pending = None;
                match response {
                    Ok(result) => self.out.set(result),
                    Err(err) => {
                        self.set_state(BlockState::fault(format!("Request: {err}")));
                    }
                }
            }
            _ = sleep_millis(timeout_millis) => {
                self.pending = None;
                let err = ConnectorError::Timeout {
                    address,
                    millis: timeout_millis,
                };
                self.set_state(BlockState::fault(format!("Request: {err}")));
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
    use crate::blocks::external::Request;
    use crate::blocks::external::support::mock::MockConnector;

    fn unique_name(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn write_all(block: &mut Request, connector: &str, value: Value) {
        write_block_inputs([
            (&mut block.input, value),
            (&mut block.connector, Value::make_str(connector)),
            (&mut block.address, Value::make_str("route")),
        ])
        .await;
    }

    #[tokio::test]
    async fn response_echoes_to_out() {
        let name = unique_name("req-flow");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;

        let requests = mock.requests.lock().unwrap().clone();
        assert_eq!(requests, vec![("route".to_string(), 42.into())]);
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn slow_response_times_out() {
        let name = unique_name("req-timeout");
        let mock = Arc::new(MockConnector {
            request_delay_millis: Some(200),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(50))]).await;
        block.execute().await;

        assert!(block.state().is_fault());
        assert_eq!(block.out.value, Value::Null, "no response on timeout");

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn request_error_faults() {
        let name = unique_name("req-err");
        let mock = Arc::new(MockConnector {
            fail_request: true,
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;

        assert!(block.state().is_fault());
        assert_eq!(block.out.value, Value::Null, "no response on failure");

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn timeout_pin_overrides_default() {
        let name = unique_name("req-override");
        let mock = Arc::new(MockConnector {
            request_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;
        write_block_inputs([(&mut block.timeout, Value::from(1000))]).await;
        block.execute().await;

        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn zero_timeout_falls_back_to_default() {
        let name = unique_name("req-zero-timeout");
        let mock = Arc::new(MockConnector {
            request_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = Request::new();
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
        let name = unique_name("req-neg-timeout");
        let mock = Arc::new(MockConnector {
            request_delay_millis: Some(20),
            ..Default::default()
        });
        register_connector(&name, mock).expect("registered");

        let mut block = Request::new();
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
    async fn config_rewrite_does_not_refire() {
        let name = unique_name("req-rebind");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;
        block.execute().await;
        assert_eq!(mock.requests.lock().unwrap().len(), 1);

        write_block_inputs([(&mut block.timeout, Value::from(1000))]).await;
        block.execute().await;

        assert_eq!(
            mock.requests.lock().unwrap().len(),
            1,
            "a config-only pin write does not re-send the cached input"
        );
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn cancelled_request_is_retried() {
        let name = unique_name("req-retry");
        let mock = Arc::new(MockConnector {
            request_delay_millis: Some(50),
            ..Default::default()
        });
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = Request::new();
        write_all(&mut block, &name, 42.into()).await;

        // Drop `execute` mid-request, as the block actor does when a
        // mailbox command arrives.
        {
            let fut = block.execute();
            tokio::pin!(fut);
            tokio::select! {
                _ = &mut fut => panic!("request should still be in flight"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {}
            }
        }
        assert_eq!(
            mock.requests.lock().unwrap().len(),
            1,
            "first attempt reached the connector"
        );
        assert_eq!(block.out.value, Value::Null);

        block.execute().await;

        assert_eq!(
            mock.requests.lock().unwrap().len(),
            2,
            "cancelled attempt is re-issued without fresh input"
        );
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }

    #[tokio::test]
    async fn missing_connector_faults() {
        let mut block = Request::new();
        write_all(&mut block, &unique_name("req-missing"), 42.into()).await;
        block.execute().await;
        assert!(block.state().is_fault());
    }

    /// The engine's `WriteInput` mailbox command writes the `in` cache
    /// directly — no watch traffic — and the command's own arrival is
    /// what cancels the in-flight `execute`, so the write always lands
    /// between two `execute` calls. The next `execute` must still
    /// treat the value as fresh and send the request.
    #[tokio::test]
    async fn cache_write_between_executes_sends() {
        use crate::base::Status;
        use crate::base::input::Input;

        let name = unique_name("req-cache-write");
        let mock = Arc::new(MockConnector::default());
        register_connector(&name, mock.clone()).expect("registered");

        let mut block = Request::new();
        write_block_inputs([
            (&mut block.connector, Value::make_str(&name)),
            (&mut block.address, Value::make_str("route")),
        ])
        .await;

        // Mimic `BlockMailboxCmd::WriteInput`: a direct cache write on
        // the `in` pin, invisible to the watch machinery.
        block.input.set_value(42.into(), Status::Ok);

        tokio::time::timeout(Duration::from_secs(1), block.execute())
            .await
            .expect("execute sends the cache-written value instead of parking");

        assert_eq!(
            mock.requests.lock().unwrap().clone(),
            vec![("route".to_string(), 42.into())],
            "a cache-written `in` value is sent"
        );
        assert_eq!(block.out.value, 42.into());
        assert!(!block.state().is_fault());

        unregister_connector(&name);
    }
}
