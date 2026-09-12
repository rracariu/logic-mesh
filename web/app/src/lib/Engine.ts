import type {
  BlockDesc,
  BlockNotification,
  EngineCommand,
  EngineSession,
} from 'logic-mesh';
import { createEngineSession } from 'logic-mesh';
import { registerUiConnector } from './UiConnector';
import { widgetBlockDescs } from './Widgets';

let session: EngineSession;
let blocks: BlockDesc[];
let command: EngineCommand;

// Serializes every call on the shared `command` handle through one
// promise chain — the same idiom as UiConnector's `attachChain` and
// EngineSession's `controlChain`. Overlapping in-flight calls on ONE
// wasm handle trip the borrow guard ("recursive use of an object
// detected"): e.g. a second example load whose `loadProgram` fires
// while the first load's round trip still holds the handle, or a
// palette drop's `addBlock` landing mid-load. Engine messages are
// FIFO, so queueing a call behind the previous one is always safe —
// the chain makes the guard unreachable by construction instead of by
// call-site discipline. Each caller receives its own call's result or
// rejection unchanged; only the NEXT link swallows a predecessor's
// rejection (it belongs to that caller, and is spent for chaining).
//
// A never-resolving call routed through the chain would wedge every
// later call forever. The one such method, `createWatch`, is never
// called on this handle anywhere in the app — watches go through
// `session.watch`'s dedicated pre-created handles.
let commandChain: Promise<unknown> = Promise.resolve();

function serializeCommand(handle: EngineCommand): EngineCommand {
  return new Proxy(handle, {
    get(target, prop) {
      const member = Reflect.get(target, prop) as unknown;
      // Every EngineCommand method is async except `free()` and
      // `[Symbol.dispose]()` (both unused by the app); chaining a sync
      // call would wrap its result in a promise, so those pass through
      // unchained. Methods must be invoked on the raw wasm instance —
      // wasm-bindgen reads the pointer off `this`.
      if (typeof member !== 'function') return member;
      if (prop === 'free' || typeof prop === 'symbol') {
        return member.bind(target) as unknown;
      }
      return (...args: unknown[]) => {
        const next = commandChain
          .catch(() => {})
          .then(() =>
            (member as (...a: unknown[]) => Promise<unknown>).apply(
              target,
              args,
            ),
          );
        commandChain = next;
        return next;
      };
    },
  });
}

export function useEngine() {
  if (!session) {
    // The session pre-creates every command handle — the general one,
    // the dedicated connector-attach handle, and the watch slots —
    // before the engine can run, which is the only time handles can be
    // created (see the EngineSession docs in the logic-mesh package
    // for the wasm borrow trap the ordering avoids). The engine itself
    // is started later, from the page's onMount, via `start()`.
    // Two watch slots: the page's onMount consumes one, and a dev-time
    // HMR remount re-runs onMount against this same module-level
    // session, consuming another — one slot would make the remount
    // throw.
    session = createEngineSession({ watchSlots: 2 });
    command = serializeCommand(session.command);
    registerUiConnector();
    blocks = [...session.engine.listBlocks(), ...widgetBlockDescs];
  }

  function startWatch(callback: (notification: BlockNotification) => void) {
    try {
      session.watch(callback);
    } catch (err) {
      // Running out of pre-created watch slots is a dev-ergonomics
      // condition, not a load-bearing failure: repeated HMR remounts
      // re-running onMount are the likely cause, and the earlier
      // watches keep delivering notifications. Warn instead of letting
      // onMount throw.
      console.warn(
        'Engine: no watch slot left (HMR remounts re-running onMount are the likely cause); keeping the existing watches:',
        err,
      );
    }
  }

  // Kicks off the engine's message loop, un-awaited — the session owns
  // the ordering rules and marks itself started, so a later startWatch
  // draws from the pre-created watch slots instead of touching the
  // (now off-limits) engine object.
  function start() {
    session.start();
  }

  // Reset must go through the session's private control handle (which
  // `session.reset()` serializes on its control chain) — never through
  // the shared `command` handle: calls like `loadProgram` hold that
  // handle's wasm borrow across a full request/reply round trip, and a
  // reset issued on the same handle mid-flight throws "recursive use
  // of an object detected". Resolves when the Reset is merely enqueued,
  // which suffices for callers: engine messages are FIFO, so a
  // request/reply ISSUED after this resolves is ordered after the
  // reset (one already in flight is not).
  function reset(): Promise<void> {
    return session.reset();
  }

  return {
    start,
    reset,
    blocks,
    // The serialized wrapper, not the raw handle — every consumer
    // (+page.svelte, model.svelte.ts, Program.ts, ToolBar.svelte) gets
    // the overlap-proof one. `connectorCommand` stays raw: its only
    // caller, `attachUiConnector`, already serializes on `attachChain`.
    command,
    connectorCommand: session.connectorCommand,
    startWatch,
  };
}
