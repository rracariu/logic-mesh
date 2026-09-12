// Copyright (c) 2022-2026, Radu Racariu.

//! Connector lifecycle helpers shared by the engines.
//!
//! Both engines track the connectors they manage as a plain list of
//! names — the handles themselves live in the process-wide connector
//! registry, the single source of truth blocks also resolve against.
//! These free functions implement the lifecycle transitions on that
//! list, so the single- and multi-threaded engines share identical
//! semantics (timeouts, ordering, and error wording) by construction.

use crate::base::connector::{ConnectorHandle, get_connector, unregister_connector};
use crate::tokio_impl::sleep::sleep_millis;

/// Deadline for a connector `start`/`stop` await, in milliseconds. A
/// connector that blows through it is logged and skipped, so a hung
/// implementation holds up `run()`, shutdown, or reset for a bounded
/// time only — not zero: `run()` entry serializes up to one timeout
/// per connector, and an attach's failure path can hold the dispatch
/// loop for up to two (a timed-out `start` followed by the cleanup
/// `stop`).
pub(super) const CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS: u64 = 5000;

/// Awaits `start` for every engine-managed connector. Failures and
/// timeouts are logged and skipped.
pub(super) async fn start_connectors(connectors: &[String]) {
    for name in connectors {
        let Some(handle) = get_connector(name) else {
            log::error!("Connector '{name}' is engine-managed but not registered");
            continue;
        };
        if let Err(err) = start_connector(name, &handle).await {
            log::error!("{err}");
        }
    }
}

/// Awaits `start` on one connector handle, bounded by
/// [`CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS`] so a hung connector cannot
/// wedge the caller. Failures and timeouts are returned as an error
/// message.
pub(super) async fn start_connector(name: &str, handle: &ConnectorHandle) -> Result<(), String> {
    tokio::select! {
        result = handle.start() => {
            result.map_err(|err| format!("Connector '{name}' failed to start: {err}"))
        }
        _ = sleep_millis(CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS) => {
            Err(format!(
                "Connector '{name}' did not start within \
                 {CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS} ms"
            ))
        }
    }
}

/// Awaits `stop` for every engine-managed connector. Connectors own
/// their IO tasks on the ambient runtime, so `stop` never needs an
/// engine-driven executor to progress. Failures and timeouts are
/// logged and skipped. The connectors stay registered — a shutdown
/// keeps the bindings for a re-run.
pub(super) async fn stop_connectors(connectors: &[String]) {
    for name in connectors {
        let Some(handle) = get_connector(name) else {
            continue;
        };
        stop_connector(name, &handle).await;
    }
}

/// Awaits `stop` on one connector handle, bounded by
/// [`CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS`] so a hung connector cannot
/// wedge shutdown or reset. Failures and timeouts are logged.
pub(super) async fn stop_connector(name: &str, handle: &ConnectorHandle) {
    tokio::select! {
        result = handle.stop() => {
            if let Err(err) = result {
                log::error!("Connector '{name}' failed to stop: {err}");
            }
        }
        _ = sleep_millis(CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS) => {
            log::error!(
                "Connector '{name}' did not stop within \
                 {CONNECTOR_LIFECYCLE_TIMEOUT_MILLIS} ms"
            );
        }
    }
}

/// Attaches an already-registered connector to a running engine:
/// resolves `name` in the process-wide connector registry, awaits the
/// connector's `start` (bounded by the lifecycle timeout), and appends
/// it to the engine's managed list.
///
/// This is the running-phase counterpart of the engines'
/// `add_connector`, reachable over the engine message channel
/// ([`AddConnectorReq`](crate::base::engine::messages::EngineMessage::AddConnectorReq)).
/// Handles cannot ride in engine messages, so callers register the
/// handle first and attach it here by name.
///
/// A registered connector must be attached to at most one engine: the
/// registry carries no ownership mark, so a second engine attaching
/// the same name would double-start the connector and its reset would
/// unregister the name out from under the first engine.
///
/// # Errors
///
/// Returns an error message if the name is not registered, is already
/// engine-managed, or the connector fails to start (or times out). On
/// a start failure the connector's `stop` is awaited before returning
/// — a failing `start` may have spawned IO tasks before erroring (or
/// completed after the timeout cancelled the await), and nothing else
/// would ever stop an untracked connector. The name stays registered,
/// so a retry remains possible.
pub(super) async fn attach_connector(
    connectors: &mut Vec<String>,
    name: String,
) -> Result<String, String> {
    if connectors.contains(&name) {
        return Err(format!(
            "Connector '{name}' is already managed by the engine"
        ));
    }
    let Some(handle) = get_connector(&name) else {
        return Err(format!("Connector '{name}' is not registered"));
    };
    if let Err(err) = start_connector(&name, &handle).await {
        stop_connector(&name, &handle).await;
        return Err(err);
    }
    connectors.push(name.clone());
    Ok(name)
}

/// Detaches an engine-managed connector: removes it from the managed
/// list, unregisters it from the process-wide registry, and then
/// awaits its `stop` — the same ordering as a reset, so no block can
/// resolve a connector that is being stopped. Stop failures and
/// timeouts are logged, not fatal.
///
/// Reachable over the engine message channel
/// ([`RemoveConnectorReq`](crate::base::engine::messages::EngineMessage::RemoveConnectorReq)).
///
/// # Errors
///
/// Returns an error message if `name` is not engine-managed.
pub(super) async fn detach_connector(
    connectors: &mut Vec<String>,
    name: &str,
) -> Result<String, String> {
    let Some(position) = connectors.iter().position(|n| n == name) else {
        return Err(format!("Connector '{name}' is not managed by the engine"));
    };
    connectors.remove(position);
    if let Some(handle) = unregister_connector(name) {
        stop_connector(name, &handle).await;
    } else {
        log::warn!(
            "Connector '{name}' was engine-managed but already \
             unregistered externally; nothing to stop"
        );
    }
    Ok(name.to_string())
}

/// Reset transition: unregister every managed connector first — no
/// block can `get_connector` a connector that is being stopped — then
/// stop the handles the unregistration handed back, and clear the
/// managed list.
pub(super) async fn unregister_and_stop_connectors(connectors: &mut Vec<String>) {
    let handles: Vec<(String, ConnectorHandle)> = std::mem::take(connectors)
        .into_iter()
        .filter_map(|name| unregister_connector(&name).map(|handle| (name, handle)))
        .collect();
    for (name, handle) in handles {
        stop_connector(&name, &handle).await;
    }
}

/// Drop transition: unregister every still-tracked connector so a
/// dropped engine does not leave the process-wide registry holding
/// entries that would reject re-registration with `AlreadyRegistered`.
///
/// `stop` is not awaited — `Drop` cannot await; connectors own their
/// IO tasks and wind those down from their own `Drop` once the
/// registry releases the last handle.
pub(super) fn unregister_connectors(connectors: &mut Vec<String>) {
    for name in std::mem::take(connectors) {
        unregister_connector(&name);
    }
}

#[cfg(test)]
pub(super) mod test_support {
    //! Connector doubles shared by the single- and multi-threaded
    //! engine lifecycle tests.

    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use libhaystack::val::Value;

    use crate::base::connector::{Connector, ConnectorFuture, ValueStream};
    use crate::base::error::ConnectorError;

    /// Records `start`/`stop` calls so tests can observe the engine
    /// driving the lifecycle.
    pub(crate) struct FlagConnector {
        pub(crate) started: Arc<AtomicBool>,
        pub(crate) stopped: Arc<AtomicBool>,
    }

    impl Connector for FlagConnector {
        fn start(&self) -> ConnectorFuture<'_, ()> {
            // Flag inside the future — the test then proves the
            // engine actually awaited it, not just created it.
            let started = self.started.clone();
            Box::pin(async move {
                started.store(true, Ordering::SeqCst);
                Ok(())
            })
        }

        fn stop(&self) -> ConnectorFuture<'_, ()> {
            let stopped = self.stopped.clone();
            Box::pin(async move {
                stopped.store(true, Ordering::SeqCst);
                Ok(())
            })
        }

        fn subscribe(&self, _address: &str) -> ConnectorFuture<'_, ValueStream> {
            Box::pin(async { Ok(Box::pin(futures::stream::empty()) as ValueStream) })
        }

        fn publish(&self, _address: &str, _value: Value) -> ConnectorFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }

        fn request(&self, _address: &str, value: Value) -> ConnectorFuture<'_, Value> {
            Box::pin(async move { Ok(value) })
        }
    }

    /// `start` always fails; `stop` records that it ran.
    pub(crate) struct FailingStartConnector {
        pub(crate) stopped: Arc<AtomicBool>,
    }

    impl Connector for FailingStartConnector {
        fn start(&self) -> ConnectorFuture<'_, ()> {
            Box::pin(async { Err(ConnectorError::Transport("start refused".into())) })
        }

        fn stop(&self) -> ConnectorFuture<'_, ()> {
            let stopped = self.stopped.clone();
            Box::pin(async move {
                stopped.store(true, Ordering::SeqCst);
                Ok(())
            })
        }

        fn subscribe(&self, _address: &str) -> ConnectorFuture<'_, ValueStream> {
            Box::pin(async { Ok(Box::pin(futures::stream::empty()) as ValueStream) })
        }

        fn publish(&self, _address: &str, _value: Value) -> ConnectorFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }

        fn request(&self, _address: &str, value: Value) -> ConnectorFuture<'_, Value> {
            Box::pin(async move { Ok(value) })
        }
    }
}
