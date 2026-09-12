// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Protocol-agnostic integration of external data sources and sinks.
//!
//! A [`Connector`] adapts one external protocol — MQTT, HTTP,
//! WebSocket, or anything else — to three primitive operations the
//! engine's external blocks build on: subscribing to a stream of
//! values, publishing a value, and a request/response round trip.
//! Protocol implementations live outside this crate; the engine only
//! sees the trait.
//!
//! Connectors are registered process-wide by name via
//! [`register_connector`], mirroring how blocks are registered: blocks
//! are moved by value into their actor tasks, so an external block
//! resolves its connector by name through [`get_connector`] at
//! execution time.
//!

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use futures::Stream;
use libhaystack::val::Value;

use super::error::ConnectorError;

/// A stream of values produced by a [`Connector`] subscription.
///
/// Dropping the stream ends the subscription — there is no explicit
/// unsubscribe call, so implementations should release any
/// protocol-level subscription from the stream's `Drop`.
///
/// Callers race the stream's `next()` against timers and mailbox
/// commands, so a `poll_next` that has internally dequeued an item
/// must keep it buffered until a poll actually yields it — an
/// implementation that dequeues and then discards the item when the
/// poll is abandoned loses values. Channel receivers already behave
/// this way.
///
/// The `Sync` bound exists because blocks hold the stream as a plain
/// struct field, and native block registration requires blocks to be
/// `Send + Sync`; channel receivers such as `tokio::sync::mpsc` and
/// `futures::channel::mpsc` satisfy it already.
#[cfg(not(target_arch = "wasm32"))]
pub type ValueStream =
    Pin<Box<dyn Stream<Item = Result<Value, ConnectorError>> + Send + Sync + 'static>>;

/// A stream of values produced by a [`Connector`] subscription.
///
/// Dropping the stream ends the subscription — there is no explicit
/// unsubscribe call, so implementations should release any
/// protocol-level subscription from the stream's `Drop`.
///
/// Callers race the stream's `next()` against timers and mailbox
/// commands, so a `poll_next` that has internally dequeued an item
/// must keep it buffered until a poll actually yields it — an
/// implementation that dequeues and then discards the item when the
/// poll is abandoned loses values. Channel receivers already behave
/// this way.
///
/// On `wasm32` the stream carries no `Send + Sync` bounds, as the host
/// is single-threaded and connectors may hold `js_sys` types.
#[cfg(target_arch = "wasm32")]
pub type ValueStream = Pin<Box<dyn Stream<Item = Result<Value, ConnectorError>> + 'static>>;

/// The boxed future every [`Connector`] operation returns.
#[cfg(not(target_arch = "wasm32"))]
pub type ConnectorFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ConnectorError>> + Send + 'a>>;

/// The boxed future every [`Connector`] operation returns.
///
/// On `wasm32` the future carries no `Send` bound, as the host is
/// single-threaded and connectors may hold `js_sys` types.
#[cfg(target_arch = "wasm32")]
pub type ConnectorFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ConnectorError>> + 'a>>;

/// A protocol adapter that exchanges values with an external system.
///
/// Implementations take `&self` — a connector handle is shared between
/// every block bound to it, so mutable connection state needs interior
/// mutability. Long-running IO loops are the connector's own concern:
/// spawn them from [`start`](Connector::start) with `tokio::spawn` (or
/// `wasm_bindgen_futures::spawn_local` on `wasm32`) and wind them down
/// in [`stop`](Connector::stop).
///
/// # Cancellation
///
/// Every future a connector returns may be dropped before it
/// completes: block actors race their block's `execute()` against the
/// actor mailbox and drop the in-flight future when an engine command
/// arrives. Dropping a future must leave the connector in a usable
/// state, and a dropped [`request`](Connector::request) should abort
/// the in-flight operation where the protocol allows. A dropped
/// [`subscribe`](Connector::subscribe) future must release any
/// protocol subscription it already established — nobody will ever
/// hold the stream, so nothing else can end it. The engine also drops
/// a [`start`](Connector::start) that outruns its lifecycle deadline;
/// that must leave the connector in a state where the
/// [`stop`](Connector::stop) the engine issues afterwards —
/// immediately on a failed attach, or at shutdown/reset for a start
/// driven from `run()` — fully winds it down.
///
/// # Examples
///
/// ```no_run
/// use futures::stream;
/// use logic_mesh::Value;
/// use logic_mesh::base::connector::{Connector, ConnectorFuture, ValueStream};
///
/// /// Answers every operation locally, without any real transport.
/// struct Echo;
///
/// impl Connector for Echo {
///     fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream> {
///         let address = address.to_string();
///         Box::pin(async move {
///             let stream = stream::iter([Ok(Value::make_str(&address))]);
///             Ok(Box::pin(stream) as ValueStream)
///         })
///     }
///
///     fn publish(&self, _address: &str, _value: Value) -> ConnectorFuture<'_, ()> {
///         Box::pin(async { Ok(()) })
///     }
///
///     fn request(&self, _address: &str, value: Value) -> ConnectorFuture<'_, Value> {
///         Box::pin(async move { Ok(value) })
///     }
/// }
/// ```
#[cfg(not(target_arch = "wasm32"))]
pub trait Connector: Send + Sync {
    /// Establishes the connector's transport. Called by the engine when
    /// it starts running. The default does nothing.
    ///
    /// Implementations must support a restart: after an engine shutdown
    /// the connector stays registered, and re-running the engine calls
    /// `start` again on the already-[`stop`](Connector::stop)ped
    /// connector.
    fn start(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Winds down the connector's transport. Called by the engine on
    /// shutdown and reset. The default does nothing.
    ///
    /// Blocks may still hold subscription streams from before the stop,
    /// so implementations should end every outstanding stream (yield
    /// [`None`]) when stopping — stream holders then observe the
    /// termination and re-subscribe after a restart.
    fn stop(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Subscribes to the values published at `address`. Dropping the
    /// returned stream ends the subscription.
    fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream>;

    /// Publishes `value` to `address`.
    fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()>;

    /// Sends `value` to `address` and resolves with the response.
    fn request(&self, address: &str, value: Value) -> ConnectorFuture<'_, Value>;
}

/// A protocol adapter that exchanges values with an external system.
///
/// Implementations take `&self` — a connector handle is shared between
/// every block bound to it, so mutable connection state needs interior
/// mutability. Long-running IO loops are the connector's own concern:
/// spawn them from [`start`](Connector::start) with
/// `wasm_bindgen_futures::spawn_local` and wind them down in
/// [`stop`](Connector::stop).
///
/// On `wasm32` the trait carries no `Send + Sync` supertraits, so
/// host-provided connectors holding `js_sys` types can implement it.
///
/// # Cancellation
///
/// Every future a connector returns may be dropped before it
/// completes: block actors race their block's `execute()` against the
/// actor mailbox and drop the in-flight future when an engine command
/// arrives. Dropping a future must leave the connector in a usable
/// state, and a dropped [`request`](Connector::request) should abort
/// the in-flight operation where the protocol allows. A dropped
/// [`subscribe`](Connector::subscribe) future must release any
/// protocol subscription it already established — nobody will ever
/// hold the stream, so nothing else can end it. The engine also drops
/// a [`start`](Connector::start) that outruns its lifecycle deadline;
/// that must leave the connector in a state where the
/// [`stop`](Connector::stop) the engine issues afterwards —
/// immediately on a failed attach, or at shutdown/reset for a start
/// driven from `run()` — fully winds it down.
#[cfg(target_arch = "wasm32")]
pub trait Connector {
    /// Establishes the connector's transport. Called by the engine when
    /// it starts running. The default does nothing.
    ///
    /// Implementations must support a restart: after an engine shutdown
    /// the connector stays registered, and re-running the engine calls
    /// `start` again on the already-[`stop`](Connector::stop)ped
    /// connector.
    fn start(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Winds down the connector's transport. Called by the engine on
    /// shutdown and reset. The default does nothing.
    ///
    /// Blocks may still hold subscription streams from before the stop,
    /// so implementations should end every outstanding stream (yield
    /// [`None`]) when stopping — stream holders then observe the
    /// termination and re-subscribe after a restart.
    fn stop(&self) -> ConnectorFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    /// Subscribes to the values published at `address`. Dropping the
    /// returned stream ends the subscription.
    fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream>;

    /// Publishes `value` to `address`.
    fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()>;

    /// Sends `value` to `address` and resolves with the response.
    fn request(&self, address: &str, value: Value) -> ConnectorFuture<'_, Value>;
}

/// The shared-ownership pointer a [`ConnectorHandle`] wraps:
/// [`std::sync::Arc`] natively, [`std::rc::Rc`] on `wasm32` (the
/// single-threaded host never sends handles across threads).
#[cfg(not(target_arch = "wasm32"))]
type HandleRepr = std::sync::Arc<dyn Connector>;

/// The shared-ownership pointer a [`ConnectorHandle`] wraps:
/// [`std::sync::Arc`] natively, [`std::rc::Rc`] on `wasm32` (the
/// single-threaded host never sends handles across threads).
#[cfg(target_arch = "wasm32")]
type HandleRepr = std::rc::Rc<dyn Connector>;

/// A shared, cheaply-cloneable handle to a registered [`Connector`].
///
/// The handle owns the connector behind the target's shared-ownership
/// pointer — `Arc` natively, `Rc` on `wasm32` — chosen in exactly one
/// place, so call sites never name it and a change of threading model
/// touches only this type. Build one with [`ConnectorHandle::new`];
/// the handle dereferences to the trait, so connector operations are
/// called on it directly.
#[derive(Clone)]
pub struct ConnectorHandle(HandleRepr);

impl ConnectorHandle {
    /// Wraps `connector` in a new shared handle.
    pub fn new(connector: impl Connector + 'static) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        return Self(std::sync::Arc::new(connector));

        #[cfg(target_arch = "wasm32")]
        Self(std::rc::Rc::new(connector))
    }
}

impl std::ops::Deref for ConnectorHandle {
    type Target = dyn Connector;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}

/// For callers that keep shared access to the concrete connector —
/// e.g. a test poking at the connector while a handle to it is
/// registered.
#[cfg(not(target_arch = "wasm32"))]
impl<T: Connector + 'static> From<std::sync::Arc<T>> for ConnectorHandle {
    fn from(connector: std::sync::Arc<T>) -> Self {
        Self(connector)
    }
}

/// For callers that keep shared access to the concrete connector —
/// e.g. a test poking at the connector while a handle to it is
/// registered.
#[cfg(target_arch = "wasm32")]
impl<T: Connector + 'static> From<std::rc::Rc<T>> for ConnectorHandle {
    fn from(connector: std::rc::Rc<T>) -> Self {
        Self(connector)
    }
}

#[cfg(not(target_arch = "wasm32"))]
static CONNECTORS: std::sync::LazyLock<std::sync::RwLock<HashMap<String, ConnectorHandle>>> =
    std::sync::LazyLock::new(|| std::sync::RwLock::new(HashMap::new()));

#[cfg(target_arch = "wasm32")]
thread_local! {
    static CONNECTORS: std::cell::RefCell<HashMap<String, ConnectorHandle>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Registers `handle` under `name` in the process-wide connector
/// registry.
///
/// # Errors
///
/// Returns [`ConnectorError::AlreadyRegistered`] if a connector with
/// this name exists.
pub fn register_connector(
    name: &str,
    handle: impl Into<ConnectorHandle>,
) -> Result<(), ConnectorError> {
    let handle = handle.into();
    let insert = |map: &mut HashMap<String, ConnectorHandle>| {
        if map.contains_key(name) {
            return Err(ConnectorError::AlreadyRegistered {
                name: name.to_string(),
            });
        }
        map.insert(name.to_string(), handle);
        Ok(())
    };

    #[cfg(not(target_arch = "wasm32"))]
    return insert(&mut CONNECTORS.write().expect("connector registry poisoned"));

    #[cfg(target_arch = "wasm32")]
    CONNECTORS.with_borrow_mut(insert)
}

/// Removes and returns the connector registered under `name`, or
/// [`None`] if there is no such connector.
pub fn unregister_connector(name: &str) -> Option<ConnectorHandle> {
    #[cfg(not(target_arch = "wasm32"))]
    return CONNECTORS
        .write()
        .expect("connector registry poisoned")
        .remove(name);

    #[cfg(target_arch = "wasm32")]
    CONNECTORS.with_borrow_mut(|map| map.remove(name))
}

/// Looks up the connector registered under `name`.
pub fn get_connector(name: &str) -> Option<ConnectorHandle> {
    #[cfg(not(target_arch = "wasm32"))]
    return CONNECTORS
        .read()
        .expect("connector registry poisoned")
        .get(name)
        .cloned();

    #[cfg(target_arch = "wasm32")]
    CONNECTORS.with_borrow(|map| map.get(name).cloned())
}

/// The names of all registered connectors.
pub fn list_connectors() -> Vec<String> {
    #[cfg(not(target_arch = "wasm32"))]
    return CONNECTORS
        .read()
        .expect("connector registry poisoned")
        .keys()
        .cloned()
        .collect();

    #[cfg(target_arch = "wasm32")]
    CONNECTORS.with_borrow(|map| map.keys().cloned().collect())
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use super::*;

    struct NoopConnector;

    impl Connector for NoopConnector {
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

    /// The registry is process-global and tests run in parallel, so
    /// every test uses a unique name.
    fn unique_name(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    fn handle() -> ConnectorHandle {
        ConnectorHandle::new(NoopConnector)
    }

    #[test]
    fn register_and_lookup() {
        let name = unique_name("lookup");

        assert!(get_connector(&name).is_none());
        register_connector(&name, handle()).expect("first registration succeeds");
        assert!(get_connector(&name).is_some());
        assert!(list_connectors().contains(&name));

        unregister_connector(&name);
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        let name = unique_name("duplicate");

        register_connector(&name, handle()).expect("first registration succeeds");
        let err = register_connector(&name, handle()).expect_err("second registration fails");
        assert_matches!(err, ConnectorError::AlreadyRegistered { name: n } if n == name);

        unregister_connector(&name);
    }

    #[test]
    fn unregister_removes_the_connector() {
        let name = unique_name("unregister");

        register_connector(&name, handle()).expect("registration succeeds");
        assert!(unregister_connector(&name).is_some());
        assert!(get_connector(&name).is_none());
        assert!(unregister_connector(&name).is_none());
    }

    #[tokio::test]
    async fn default_start_and_stop_succeed() {
        let connector = NoopConnector;
        connector.start().await.expect("default start is Ok");
        connector.stop().await.expect("default stop is Ok");
    }
}
