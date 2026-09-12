import type { ConnectorCallback, EngineCommand, JsConnector } from 'logic-mesh';
import {
  connectorIs,
  connectorRegistered,
  registerConnector,
  unregisterConnector,
} from 'logic-mesh';
import type { Block } from './Block';

// The connector every UI widget exchanges values through. Input widgets
// push into it and ExternalIn blocks subscribe; ExternalOut blocks
// publish and display widgets listen. Addresses are block UUIDs.
export const UI_CONNECTOR_NAME = 'ui';

interface Subscription {
  callback: ConnectorCallback;
  live: boolean;
}

class UiConnector implements JsConnector {
  // Engine-side subscriptions (ExternalIn), keyed by address.
  private subscriptions = new Map<string, Set<Subscription>>();

  // Latest value pushed from the UI per address, flushed to late
  // subscribers so widgets placed before the engine subscribes still
  // deliver their initial value.
  private pushed = new Map<string, unknown>();

  // App-side listeners (display widgets), keyed by address.
  private listeners = new Map<string, Set<(value: unknown) => void>>();

  // Latest value published by the engine per address, flushed to late
  // listeners so a widget mounted after a publish shows the value.
  private published = new Map<string, unknown>();

  subscribe(address: string, callback: ConnectorCallback) {
    const sub: Subscription = { callback, live: true };
    let set = this.subscriptions.get(address);
    if (!set) {
      set = new Set();
      this.subscriptions.set(address, set);
    }
    set.add(sub);

    if (this.pushed.has(address)) {
      callback(this.pushed.get(address));
    }

    // Idempotent; must never let the callback fire afterwards.
    return () => {
      sub.live = false;
      set.delete(sub);
      // Drop the address entry with its last subscription — the
      // identity check guards against deleting a set a later
      // subscribe re-created under the same address.
      if (set.size === 0 && this.subscriptions.get(address) === set) {
        this.subscriptions.delete(address);
      }
    };
  }

  publish(address: string, value: unknown) {
    this.published.set(address, value);
    const listeners = this.listeners.get(address);
    if (listeners) {
      for (const listener of listeners) listener(value);
    }
  }

  request(address: string): never {
    throw new Error(
      `the '${UI_CONNECTOR_NAME}' connector has no services (requested '${address}')`,
    );
  }

  stop() {
    for (const set of this.subscriptions.values()) {
      for (const sub of set) {
        sub.live = false;
        // Zero-arg call ends the stream, per the connector contract.
        sub.callback();
      }
    }
    this.subscriptions.clear();
    this.pushed.clear();
    this.published.clear();
    // App-side listeners die with the stopped connector too — a stop
    // accompanies a reset that unmounts the widgets, and their
    // teardowns tolerate the entry being gone already.
    this.listeners.clear();
  }

  // While paused (engine execution paused), pushes only update the
  // latest-value cache instead of feeding the engine an unbounded
  // backlog that would replay on resume.
  private paused = false;
  private pendingWhilePaused = new Set<string>();

  setPaused(paused: boolean) {
    this.paused = paused;
    if (paused) return;
    // Flush the latest value per address touched while paused.
    for (const address of this.pendingWhilePaused) {
      if (this.pushed.has(address)) {
        this.deliver(address, this.pushed.get(address));
      }
    }
    this.pendingWhilePaused.clear();
  }

  pushValue(address: string, value: unknown) {
    // `undefined` would silently end the subscription stream.
    if (value === undefined) return;
    this.pushed.set(address, value);
    if (this.paused) {
      this.pendingWhilePaused.add(address);
      return;
    }
    this.deliver(address, value);
  }

  private deliver(address: string, value: unknown) {
    const set = this.subscriptions.get(address);
    if (set) {
      for (const sub of set) {
        if (sub.live) sub.callback(value);
      }
    }
  }

  // Drops the cached latest values for an address whose block is gone.
  forgetAddress(address: string) {
    this.pushed.delete(address);
    this.published.delete(address);
    this.pendingWhilePaused.delete(address);
  }

  onValue(address: string, listener: (value: unknown) => void): () => void {
    let set = this.listeners.get(address);
    if (!set) {
      set = new Set();
      this.listeners.set(address, set);
    }
    set.add(listener);

    if (this.published.has(address)) {
      listener(this.published.get(address));
    }

    return () => {
      set.delete(listener);
      // Drop the address entry with its last listener — the identity
      // check guards against deleting a set a later onValue re-created
      // under the same address.
      if (set.size === 0 && this.listeners.get(address) === set) {
        this.listeners.delete(address);
      }
    };
  }
}

const uiConnector = new UiConnector();

/** Sends `value` from an input widget toward the engine. */
export function pushValue(address: string, value: unknown) {
  uiConnector.pushValue(address, value);
}

/**
 * Subscribes a display widget to values the engine publishes to
 * `address`. Returns the unsubscribe function.
 */
export function onValue(
  address: string,
  listener: (value: unknown) => void,
): () => void {
  return uiConnector.onValue(address, listener);
}

/**
 * Pauses or resumes UI pushes toward the engine. While paused, pushes
 * only update the latest-value cache; on resume the latest value per
 * touched address is flushed to live subscriptions.
 */
export function setUiPushPaused(paused: boolean) {
  uiConnector.setPaused(paused);
}

/** Drops the cached values for a removed widget block's address. */
export function forgetAddress(address: string) {
  uiConnector.forgetAddress(address);
}

/**
 * Drops cached values for a deleted ExternalIn/ExternalOut block —
 * plain ones included, or their cached publish would replay as a
 * live-looking value to later subscribers of the same address.
 */
export function forgetBlockAddress(block: Block) {
  if (block.desc.name !== 'ExternalIn' && block.desc.name !== 'ExternalOut') {
    return;
  }
  const address = block.inputs['address']?.value;
  forgetAddress(typeof address === 'string' && address ? address : block.id);
}

/**
 * Puts the UI connector in the process-wide registry. Safe to call
 * before the engine runs; registration alone does not attach it. Goes
 * through the module-level wasm exports — never engine methods, whose
 * wasm borrow is held for the engine's whole life once `run()` has
 * been polled. When the registry already holds THIS module's instance
 * (identity-checked via `connectorIs`) it is left untouched — an
 * unregister/register round trip would open a window with no `ui`
 * connector registered, during which an engine holding it attached
 * would fault every widget ExternalIn/ExternalOut. Only a foreign
 * registration — a previous module instance's connector after a Vite
 * HMR re-init, where the wasm registry outlives this module but each
 * re-init creates a fresh `uiConnector` object — is replaced.
 */
export function registerUiConnector() {
  if (connectorIs(UI_CONNECTOR_NAME, uiConnector)) return;
  if (connectorRegistered(UI_CONNECTOR_NAME)) {
    unregisterConnector(UI_CONNECTOR_NAME);
  }
  registerConnector(UI_CONNECTOR_NAME, uiConnector);
}

// Serializes attach attempts: the app resets via `session.reset()`,
// which resolves when Reset is merely enqueued, and concurrent
// attaches (initial mount + reset handler) must not interleave their
// check-then-attach sequences.
let attachChain: Promise<void> = Promise.resolve();

async function doAttach(command: EngineCommand): Promise<void> {
  // Request/reply barrier: engine messages are FIFO and this only
  // resolves after the engine processed everything queued before it —
  // including a pending Reset that unregisters attached connectors.
  const attached = await command.listConnectors();
  if (attached.includes(UI_CONNECTOR_NAME)) return;

  registerUiConnector();
  await command.addConnector(UI_CONNECTOR_NAME);
}

/**
 * Registers (tolerantly, in case it already is) and attaches the UI
 * connector to the running engine. Call after `engine.run()` and after
 * every engine reset — reset removes attached connectors from the
 * registry. Skips attaching when the connector is already attached.
 *
 * `command` must be a handle created BEFORE `engine.run()` (Engine.ts
 * exposes a dedicated `connectorCommand`): once run() is polled, its
 * future holds the engine object's wasm borrow forever, and creating a
 * handle here — from a promise continuation — would throw.
 */
export function attachUiConnector(command: EngineCommand): Promise<void> {
  const next = attachChain.catch(() => {}).then(() => doAttach(command));
  attachChain = next;
  return next;
}
