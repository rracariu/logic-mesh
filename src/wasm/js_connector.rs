// Copyright (c) 2022-2026, Radu Racariu.

//!
//! JavaScript-implemented connectors.
//!
//! Lets the JS host implement the [`Connector`] trait: a connector is
//! a JS object whose methods carry the three primitive operations (and
//! the optional lifecycle hooks), registered under a name via
//! [`registerConnector`](register_js_connector) — either the
//! module-level function or the `BlocksEngine.registerConnector`
//! convenience method. Registration only enters the process-wide
//! connector registry; attach the connector to a running engine with
//! the `addConnector` engine command
//! ([`EngineCommand::add_connector`](crate::wasm::engine_command::EngineCommand::add_connector)).
//! A connector that was registered but never attached can be removed
//! again with [`unregisterConnector`](unregister_js_connector).
//!
//! Values cross the JS boundary through `serde-wasm-bindgen` in the
//! same Haystack JSON encoding JS blocks use: plain JS numbers,
//! strings, booleans and `null` map to the corresponding
//! [`Value`] variants.
//!
//! # The connector object
//!
//! `subscribe`, `publish` and `request` are required and must be
//! functions; `start` and `stop` are optional. Every method may return
//! a `Promise` (or any thenable) — the engine awaits it — or a plain
//! value. Methods are invoked with `this` bound to the connector
//! object itself, so registering a class instance
//! (`registerConnector("mqtt", new MqttConnector(url))`) works and
//! methods may keep connection state on `this`.
//!
//! ```js
//! import { registerConnector } from "logic-mesh";
//!
//! registerConnector("demo", {
//!   // One stream-ending function per live subscription.
//!   subscriptions: new Set(),
//!
//!   // Optional. Called when the engine starts running.
//!   start() {},
//!
//!   // Optional. Called on engine shutdown and reset. Ends every
//!   // outstanding subscription by invoking its callback with no
//!   // arguments.
//!   stop() {
//!     for (const end of this.subscriptions) end();
//!     this.subscriptions.clear();
//!   },
//!
//!   // Called once per subscription. Push each new value with
//!   // `callback(value)`, report a route failure with
//!   // `callback(undefined, detail)`, and end the stream with
//!   // `callback()`. Return an unsubscribe function — directly or
//!   // via a Promise — to be told when the engine drops the
//!   // subscription; return nothing if there is no cleanup to do.
//!   subscribe(address, callback) {
//!     const timer = setInterval(() => callback(Math.random()), 1000);
//!     const end = () => {
//!       clearInterval(timer);
//!       callback();
//!     };
//!     this.subscriptions.add(end);
//!     return () => {
//!       clearInterval(timer);
//!       this.subscriptions.delete(end);
//!     };
//!   },
//!
//!   // A thrown exception or a rejected Promise marks the publish
//!   // as failed.
//!   publish(address, value) {},
//!
//!   // Resolves with the response value.
//!   async request(address, value) {
//!     return value;
//!   },
//! });
//! ```
//!
//! # The subscription callback
//!
//! The callback passed to `subscribe` takes `(value, error)`:
//!
//! * `callback(value)` — pushes `value` into the subscription stream.
//! * `callback(undefined, detail)` — pushes a
//!   [`ConnectorError::Subscribe`] carrying `detail` into the stream;
//!   the subscription stays live.
//! * `callback()` — both arguments `undefined` — ends the stream.
//!
//! **Beware:** an accidentally-`undefined` value is indistinguishable
//! from `callback()` and silently ends the subscription — guard
//! payloads on the JS side before pushing them. A value that fails to
//! convert becomes an error item in the stream rather than a panic.
//!
//! Pushed values are buffered without bound between the JS producer
//! and the consuming block, which drains roughly one item per
//! execution cycle and nothing at all while the engine is paused —
//! fast producers should throttle themselves, or stop pushing while
//! the engine is not running.
//!
//! Once the engine drops the subscription, the returned unsubscribe
//! function is invoked and the callback is destroyed — calling the
//! callback afterwards throws on the JS side, so JS implementations
//! must stop calling it once unsubscribed. The unsubscribe function
//! may also run after JS itself ended the stream with `callback()`
//! (the engine still drops the stream, which unsubscribes), so it
//! should be idempotent. If `subscribe` throws or rejects, the
//! callback is destroyed immediately — deregister it from the
//! transport before rejecting.
//!
//! # Cancellation
//!
//! Connector futures may be dropped mid-await (see the trait's
//! cancellation contract). An in-flight JS `Promise` is not — and
//! cannot be — aborted: it settles on its own and its result is
//! discarded. A `subscribe` cancelled while its Promise is pending
//! attaches a settlement handler that invokes the eventual unsubscribe
//! function, so the JS side is still wound down; the value callback
//! stays alive until that handler runs (and is leaked only if the
//! Promise never settles).
//!

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use futures::channel::mpsc;
use js_sys::Promise;
use libhaystack::val::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;

use crate::base::connector::{
    Connector, ConnectorFuture, ConnectorHandle, ValueStream, get_connector, register_connector,
    unregister_connector,
};
use crate::base::error::ConnectorError;

/// The value callback handed to the JS `subscribe` method.
///
/// Called by JS as `callback(value, error?)` — see the
/// [module docs](self) for the exact contract.
type ValueCallback = Closure<dyn FnMut(JsValue, JsValue)>;

/// A [`Connector`] implemented by a JavaScript object.
///
/// Holds the connector object and its methods as
/// [`js_sys::Function`]s, adapting each trait operation onto them —
/// every call binds `this` to the object, converts values with
/// `serde-wasm-bindgen` and awaits a returned Promise. Build one from
/// a JS object with [`JsConnector::from_js`] and register it through
/// [`registerConnector`](register_js_connector).
pub struct JsConnector {
    this: JsValue,
    subscribe_fn: js_sys::Function,
    publish_fn: js_sys::Function,
    request_fn: js_sys::Function,
    start_fn: Option<js_sys::Function>,
    stop_fn: Option<js_sys::Function>,
}

impl JsConnector {
    /// Extracts the connector methods from a JS object — see the
    /// [module docs](self) for the expected shape. The object itself
    /// is retained as the `this` receiver of every method call.
    ///
    /// # Errors
    ///
    /// Returns a message describing the failure if `connector` is not
    /// an object, if a required property (`subscribe`, `publish`,
    /// `request`) is missing or not a function, or if an optional
    /// property (`start`, `stop`) is present but not a function.
    pub fn from_js(connector: &JsValue) -> Result<Self, String> {
        Ok(Self {
            this: connector.clone(),
            subscribe_fn: required_fn(connector, "subscribe")?,
            publish_fn: required_fn(connector, "publish")?,
            request_fn: required_fn(connector, "request")?,
            start_fn: optional_fn(connector, "start")?,
            stop_fn: optional_fn(connector, "stop")?,
        })
    }
}

impl Connector for JsConnector {
    fn start(&self) -> ConnectorFuture<'_, ()> {
        call_lifecycle_fn(self.start_fn.clone(), self.this.clone(), "start")
    }

    fn stop(&self) -> ConnectorFuture<'_, ()> {
        call_lifecycle_fn(self.stop_fn.clone(), self.this.clone(), "stop")
    }

    fn subscribe(&self, address: &str) -> ConnectorFuture<'_, ValueStream> {
        let func = self.subscribe_fn.clone();
        let this = self.this.clone();
        let address = address.to_string();

        Box::pin(async move {
            let (sender, receiver) = mpsc::unbounded();
            let callback = make_value_callback(address.clone(), sender);

            let ret = func
                .call2(&this, &JsValue::from_str(&address), callback.as_ref())
                .map_err(|err| subscribe_error(&address, js_detail(&err)))?;

            // `Promise::resolve` handles a plain unsubscribe function,
            // a native Promise and any thenable uniformly.
            let promise = Promise::resolve(&ret);
            let mut cleanup = SubscribeCleanup::new(promise.clone(), callback);
            let settled = JsFuture::from(promise).await;
            let callback = cleanup.disarm();
            let resolved = settled.map_err(|err| subscribe_error(&address, js_detail(&err)))?;
            let unsubscribe = as_unsubscribe_fn(&address, resolved);

            Ok(Box::pin(JsSubscription {
                receiver,
                unsubscribe,
                _callback: callback,
            }) as ValueStream)
        })
    }

    fn publish(&self, address: &str, value: Value) -> ConnectorFuture<'_, ()> {
        let func = self.publish_fn.clone();
        let this = self.this.clone();
        let address = address.to_string();

        Box::pin(async move {
            let error = |detail: String| ConnectorError::Publish {
                address: address.clone(),
                detail,
            };

            let value =
                serde_wasm_bindgen::to_value(&value).map_err(|err| error(err.to_string()))?;
            let ret = func
                .call2(&this, &JsValue::from_str(&address), &value)
                .map_err(|err| error(js_detail(&err)))?;

            JsFuture::from(Promise::resolve(&ret))
                .await
                .map_err(|err| error(js_detail(&err)))?;

            Ok(())
        })
    }

    fn request(&self, address: &str, value: Value) -> ConnectorFuture<'_, Value> {
        let func = self.request_fn.clone();
        let this = self.this.clone();
        let address = address.to_string();

        Box::pin(async move {
            let error = |detail: String| ConnectorError::Request {
                address: address.clone(),
                detail,
            };

            let value =
                serde_wasm_bindgen::to_value(&value).map_err(|err| error(err.to_string()))?;
            let ret = func
                .call2(&this, &JsValue::from_str(&address), &value)
                .map_err(|err| error(js_detail(&err)))?;

            let response = JsFuture::from(Promise::resolve(&ret))
                .await
                .map_err(|err| error(js_detail(&err)))?;

            serde_wasm_bindgen::from_value::<Value>(response).map_err(|err| error(err.to_string()))
        })
    }
}

/// Registers a JavaScript-implemented connector under `name` in the
/// process-wide connector registry.
///
/// `connector` is a JS object carrying `subscribe`, `publish` and
/// `request` functions plus optional `start` and `stop` — see the
/// [module docs](self) for the full contract. Callable at any time,
/// including while an engine runs; attach the registered connector to
/// a running engine with the `addConnector` engine command.
///
/// Exported to JS as `registerConnector`.
///
/// # Errors
///
/// Returns (throws, on the JS side) a message if `connector` does not
/// have the required shape — see [`JsConnector::from_js`] — or if a
/// connector named `name` is already registered.
#[wasm_bindgen(js_name = "registerConnector")]
pub fn register_js_connector(name: String, connector: JsValue) -> Result<(), String> {
    let js_connector = JsConnector::from_js(&connector)?;
    register_connector(&name, ConnectorHandle::new(js_connector)).map_err(|err| err.to_string())?;
    // Record the raw object only once the registration is in — the
    // identity table must never claim a name the registry rejected.
    JS_CONNECTOR_OBJECTS.with_borrow_mut(|map| {
        map.insert(name, connector);
    });
    Ok(())
}

thread_local! {
    /// The raw JS object registered under each connector name, kept so
    /// [`connectorIs`](js_connector_is) can answer identity questions
    /// the type-erased registry cannot: a [`ConnectorHandle`] wraps a
    /// `dyn Connector` with no downcast path back to the [`JsConnector`]
    /// (and the `this` it retains). Every JS-side registration goes
    /// through [`registerConnector`](register_js_connector), which
    /// refreshes the entry, so whenever the registry holds a JS-owned
    /// name this table holds the object it was registered with. An
    /// entry can outlive its registration (engine reset and detach
    /// unregister through the core registry, which knows nothing of
    /// this table) — such stragglers are pruned lazily by
    /// [`connectorIs`](js_connector_is) once it sees the name gone.
    static JS_CONNECTOR_OBJECTS: std::cell::RefCell<
        std::collections::HashMap<String, JsValue>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Reports whether the connector currently registered under `name` is
/// a JavaScript connector wrapping exactly `connector` — compared by
/// JS object identity (`===`), the way JS itself distinguishes two
/// same-shaped objects.
///
/// This is what lets a JS façade decide *ownership* against the live
/// registry rather than its own bookkeeping: a boolean "I registered
/// this" flag goes stale the moment an engine reset unregisters the
/// connector behind the façade's back, after which the same name may
/// be re-registered by someone else. `false` therefore means either
/// "nothing is registered under `name`" or "something else is".
///
/// Exported to JS as `connectorIs`.
#[wasm_bindgen(js_name = "connectorIs")]
pub fn js_connector_is(name: &str, connector: JsValue) -> bool {
    if get_connector(name).is_none() {
        // The registration is gone (reset, detach, or explicit
        // unregister); drop the stale object reference too, so the
        // table cannot pin dead JS objects for the process lifetime.
        JS_CONNECTOR_OBJECTS.with_borrow_mut(|map| {
            map.remove(name);
        });
        return false;
    }

    JS_CONNECTOR_OBJECTS.with_borrow(|map| {
        // `JsValue::eq` is JS `===` — reference identity for objects.
        map.get(name).is_some_and(|stored| *stored == connector)
    })
}

/// Removes the connector registered under `name` from the
/// process-wide connector registry, returning whether a connector was
/// removed.
///
/// Symmetric with [`registerConnector`](register_js_connector) and
/// meant for a connector that was registered but never attached to an
/// engine. A connector attached to a running engine should instead be
/// removed with the `removeConnector` engine command, which detaches
/// and stops it before unregistering — raw unregistration leaves the
/// engine tracking a stale name (the engine copes: detaching it later
/// warns that the connector was already unregistered externally).
///
/// Exported to JS as `unregisterConnector`.
#[wasm_bindgen(js_name = "unregisterConnector")]
pub fn unregister_js_connector(name: String) -> bool {
    let removed = unregister_connector(&name).is_some();
    if removed {
        // Keep the identity table in step — see JS_CONNECTOR_OBJECTS.
        JS_CONNECTOR_OBJECTS.with_borrow_mut(|map| {
            map.remove(&name);
        });
    }
    removed
}

/// Reports whether a connector is registered under `name` in the
/// process-wide connector registry.
///
/// Lets JS distinguish "not yet registered" from "registered but
/// stale" (e.g. a registry entry that outlived a module re-init)
/// without attempting a registration just to catch the
/// already-registered error — [`registerConnector`](register_js_connector)
/// rejects duplicates, and the registry lives for the process, not the
/// module.
///
/// Exported to JS as `connectorRegistered`.
#[wasm_bindgen(js_name = "connectorRegistered")]
pub fn js_connector_registered(name: String) -> bool {
    get_connector(&name).is_some()
}

/// The stream a [`JsConnector`] subscription produces.
///
/// Owns the channel receiver the value callback feeds, the callback
/// [`Closure`] itself — kept alive exactly as long as JS may call it —
/// and the optional JS unsubscribe function. Dropping the stream first
/// invokes the unsubscribe function and then destroys the callback,
/// mapping the trait's drop-is-unsubscribe contract onto JS.
struct JsSubscription {
    receiver: mpsc::UnboundedReceiver<Result<Value, ConnectorError>>,
    unsubscribe: Option<js_sys::Function>,
    _callback: ValueCallback,
}

impl Stream for JsSubscription {
    type Item = Result<Value, ConnectorError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}

impl Drop for JsSubscription {
    fn drop(&mut self) {
        // `Drop::drop` runs before the fields are dropped, so the
        // callback closure is still alive while JS unsubscribes.
        if let Some(unsubscribe) = self.unsubscribe.take()
            && let Err(err) = unsubscribe.call0(&JsValue::NULL)
        {
            log::warn!("Connector unsubscribe function threw: {}", js_detail(&err));
        }
    }
}

/// Winds down a JS subscription whose `subscribe` Promise is still
/// pending when the future awaiting it is dropped (cancellation).
///
/// Armed around the `await` of the Promise; on the success and failure
/// paths [`disarm`](SubscribeCleanup::disarm) recovers the callback
/// and turns `Drop` into a no-op. If the future is instead dropped
/// mid-await, `Drop` attaches a settlement handler to the Promise that
/// keeps the callback alive until the Promise settles and invokes the
/// resolved unsubscribe function, if any.
struct SubscribeCleanup {
    promise: Option<Promise>,
    callback: Option<ValueCallback>,
}

impl SubscribeCleanup {
    fn new(promise: Promise, callback: ValueCallback) -> Self {
        Self {
            promise: Some(promise),
            callback: Some(callback),
        }
    }

    /// Recovers the callback and defuses the `Drop` cleanup. Must be
    /// called at most once.
    fn disarm(&mut self) -> ValueCallback {
        self.promise = None;
        self.callback
            .take()
            .expect("subscribe cleanup disarmed only once")
    }
}

impl Drop for SubscribeCleanup {
    fn drop(&mut self) {
        let (Some(promise), Some(callback)) = (self.promise.take(), self.callback.take()) else {
            return;
        };

        // A one-shot JS function that frees itself after its first
        // call; it is leaked only if the Promise never settles.
        let cleanup = Closure::once_into_js(move |settled: JsValue| {
            // Unsubscribe while the callback is still alive: a JS
            // unsubscribe may flush a final value through it or end
            // the stream with `callback()`.
            if let Ok(unsubscribe) = settled.dyn_into::<js_sys::Function>()
                && let Err(err) = unsubscribe.call0(&JsValue::NULL)
            {
                log::warn!(
                    "Connector unsubscribe function threw after a cancelled subscribe: {}",
                    js_detail(&err)
                );
            }

            drop(callback);
        });

        // Attach as both the fulfillment and the rejection handler —
        // exactly one of the two slots ever runs. Called through the
        // Promise's own `then` because the typed `js_sys` binding only
        // accepts borrowed `Closure`s, not a self-freeing function.
        if let Ok(then) = js_sys::Reflect::get(&promise, &JsValue::from_str("then"))
            && let Some(then) = then.dyn_ref::<js_sys::Function>()
            && let Err(err) = then.call2(&promise, &cleanup, &cleanup)
        {
            log::warn!(
                "Failed to attach cleanup to a cancelled subscribe Promise: {}",
                js_detail(&err)
            );
        }
    }
}

/// Builds the `(value, error)` callback a subscription hands to JS,
/// feeding `sender` — see the [module docs](self) for the JS-facing
/// contract.
fn make_value_callback(
    address: String,
    sender: mpsc::UnboundedSender<Result<Value, ConnectorError>>,
) -> ValueCallback {
    let mut sender = Some(sender);

    Closure::new(move |value: JsValue, error: JsValue| {
        let Some(tx) = sender.as_ref() else {
            return;
        };

        // `callback()` — no arguments — ends the stream by dropping
        // the sender, which terminates the receiver.
        if value.is_undefined() && error.is_undefined() {
            sender = None;
            return;
        }

        let item = if error.is_undefined() || error.is_null() {
            serde_wasm_bindgen::from_value::<Value>(value)
                .map_err(|err| subscribe_error(&address, err.to_string()))
        } else {
            Err(subscribe_error(&address, js_detail(&error)))
        };

        // A send fails only when the receiver is gone; stop trying.
        if tx.unbounded_send(item).is_err() {
            sender = None;
        }
    })
}

/// Calls an optional lifecycle function (`start`/`stop`) with `this`
/// bound to the connector object, awaiting a returned Promise. An
/// absent function succeeds immediately; exceptions and rejections map
/// to [`ConnectorError::Transport`].
fn call_lifecycle_fn(
    func: Option<js_sys::Function>,
    this: JsValue,
    name: &'static str,
) -> ConnectorFuture<'static, ()> {
    Box::pin(async move {
        let Some(func) = func else {
            return Ok(());
        };

        let error = |detail: String| {
            ConnectorError::Transport(format!("connector {name} failed: {detail}"))
        };

        let ret = func.call0(&this).map_err(|err| error(js_detail(&err)))?;

        JsFuture::from(Promise::resolve(&ret))
            .await
            .map_err(|err| error(js_detail(&err)))?;

        Ok(())
    })
}

/// Interprets what a JS `subscribe` returned (or its Promise resolved
/// to) as an unsubscribe hook: a function is the hook, `undefined` and
/// `null` mean there is none, and anything else is ignored with a
/// warning.
fn as_unsubscribe_fn(address: &str, value: JsValue) -> Option<js_sys::Function> {
    match value.dyn_into::<js_sys::Function>() {
        Ok(func) => Some(func),
        Err(value) => {
            if !value.is_undefined() && !value.is_null() {
                log::warn!(
                    "Connector subscribe for '{address}' returned a value that is neither a function nor null/undefined; ignoring it"
                );
            }
            None
        }
    }
}

/// Builds the [`ConnectorError::Subscribe`] every subscription failure
/// path reports.
fn subscribe_error(address: &str, detail: String) -> ConnectorError {
    ConnectorError::Subscribe {
        address: address.to_string(),
        detail,
    }
}

/// Renders a JS exception or rejection reason as a detail message: an
/// `Error` via its own `toString()` (preserving the error name, e.g.
/// `"TypeError: ..."`), a plain string as-is, anything else via its
/// debug representation.
fn js_detail(err: &JsValue) -> String {
    if let Some(error) = err.dyn_ref::<js_sys::Error>() {
        return String::from(js_sys::Error::to_string(error));
    }

    err.as_string().unwrap_or_else(|| format!("{err:?}"))
}

/// Reads property `name` from `connector`, requiring it to be a
/// function.
fn required_fn(connector: &JsValue, name: &str) -> Result<js_sys::Function, String> {
    let prop = js_sys::Reflect::get(connector, &JsValue::from_str(name)).map_err(|err| {
        format!(
            "Failed to read connector property '{name}': {}",
            js_detail(&err)
        )
    })?;

    prop.dyn_into::<js_sys::Function>()
        .map_err(|_| format!("Connector property '{name}' is required and must be a function"))
}

/// Reads property `name` from `connector`; absent (or `null`) is
/// [`None`], anything else must be a function.
fn optional_fn(connector: &JsValue, name: &str) -> Result<Option<js_sys::Function>, String> {
    let prop = js_sys::Reflect::get(connector, &JsValue::from_str(name)).map_err(|err| {
        format!(
            "Failed to read connector property '{name}': {}",
            js_detail(&err)
        )
    })?;

    if prop.is_undefined() || prop.is_null() {
        return Ok(None);
    }

    prop.dyn_into::<js_sys::Function>()
        .map(Some)
        .map_err(|_| format!("Connector property '{name}', when present, must be a function"))
}
