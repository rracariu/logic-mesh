import type { BlockNotification } from './index.js';
import type { BlocksEngine, EngineCommand } from './logic_mesh.js';
import { initEngine } from './logic_mesh.js';

/**
 * Options for {@link createEngineSession} and {@link startEngine}.
 */
export interface EngineSessionOptions {
  /**
   * An engine to adopt instead of creating one. It must not have been
   * run yet — the session creates its command handles from it, which
   * is impossible once `run()`'s future has been polled.
   */
  engine?: BlocksEngine;

  /**
   * Block sleep duration in milliseconds, forwarded to `initEngine`
   * when the session creates the engine itself. Ignored when an
   * `engine` is supplied.
   */
  sleepDuration?: number;

  /**
   * How many watch handles to pre-create for {@link EngineSession.watch}.
   * Each watch permanently consumes one command handle (`createWatch`
   * never resolves), and handles cannot be created after the engine
   * starts. Defaults to 1.
   */
  watchSlots?: number;
}

/**
 * Owns the engine start sequence, so the ordering rules cannot be
 * gotten wrong.
 *
 * Two wasm-bindgen facts make the manual sequence a footgun:
 *
 * 1. The future `BlocksEngine.run()` returns holds the engine object's
 *    wasm borrow for as long as the engine lives. From the moment that
 *    future is first polled — the microtask after `run()` is called —
 *    ANY method call on the engine object (`engineCommand()`,
 *    `listBlocks()`, `registerBlock()`, …) throws "recursive use of an
 *    object detected". Every command handle must therefore be created
 *    before `run()`.
 * 2. Commands are serviced only while the engine's message loop runs,
 *    and `run()`'s promise resolves only when the engine shuts down.
 *    Awaiting `run()` before issuing commands deadlocks, and so does
 *    awaiting a command before `run()` has been kicked off.
 *
 * The session bakes the correct order in: every handle — including the
 * watch slots and an internal control handle — is created up front,
 * {@link start} kicks `run()` off without awaiting it, and
 * {@link stop} / {@link reset} travel through the dedicated control
 * handle so they never touch the engine object and never collide with
 * a caller's in-flight command on a shared handle (overlapping calls
 * on one handle trip the same borrow guard).
 *
 * All handles feed the same FIFO engine queue, so a request/reply
 * command awaited on one handle (e.g. `listConnectors`) doubles as a
 * barrier proving the engine has processed everything queued before it
 * on every handle — the property connector attach flows rely on.
 *
 * Create a session with {@link startEngine} (creates, then starts) or
 * {@link createEngineSession} (handles now, {@link start} later — for
 * hosts that create the engine at module load but start it on mount).
 */
export class EngineSession {
  /**
   * The engine object. Valid for pre-start setup only —
   * `registerBlock`, `listBlocks`, `run` — and off limits from the
   * first poll of `run()`'s future onwards; use the command handles
   * instead.
   */
  readonly engine: BlocksEngine;

  /** The general-purpose command handle. */
  readonly command: EngineCommand;

  /**
   * A handle dedicated to connector attachment, so attach flows that
   * run from promise continuations (after a reset, on mount) never
   * contend with in-flight commands on {@link command}. Its
   * `listConnectors` reply is the FIFO barrier described above.
   */
  readonly connectorCommand: EngineCommand;

  /** Stop/reset handle — see the class docs. */
  private readonly control: EngineCommand;

  /** Pre-created handles {@link watch} draws from after start. */
  private readonly watchReserve: EngineCommand[];

  /** Serializes control-handle operations (stop/reset) — overlapping
   * calls on one handle trip the wasm borrow guard. */
  private controlChain: Promise<unknown> = Promise.resolve();

  private runDone: Promise<void> | undefined;

  private stopDone: Promise<void> | undefined;

  constructor(options: EngineSessionOptions = {}) {
    this.engine = options.engine ?? initEngine(options.sleepDuration);
    this.command = this.engine.engineCommand();
    this.connectorCommand = this.engine.engineCommand();
    this.control = this.engine.engineCommand();
    this.watchReserve = Array.from({ length: options.watchSlots ?? 1 }, () =>
      this.engine.engineCommand(),
    );
  }

  /** Whether {@link start} has been called. */
  get started(): boolean {
    return this.runDone !== undefined;
  }

  /**
   * Kicks off `engine.run()` without awaiting it (its promise resolves
   * only on shutdown; {@link stop} awaits it then). Idempotent. From
   * here on the engine object must not be touched — everything goes
   * through the pre-created command handles.
   *
   * Throws after {@link stop}: a stopped engine cannot be restarted —
   * without the guard the session would look started while every
   * command hangs forever against the ended message loop.
   */
  start(): void {
    if (this.stopDone) {
      throw new Error(
        'EngineSession.start() called after stop(): a stopped engine ' +
          'cannot be restarted — its message loop has ended and commands ' +
          'would hang forever. Create a new session to run again.',
      );
    }
    if (this.runDone) return;
    const run = this.engine.run();
    this.runDone = run;
    // Keep a shutdown-time rejection from surfacing as an unhandled
    // rejection when nobody ever calls stop(); stop() still sees it.
    run.catch(() => {});
  }

  /**
   * Mints an additional command handle. Only possible before
   * {@link start} — afterwards the engine's wasm borrow is held by
   * `run()`'s future and this throws a descriptive error instead of
   * the raw "recursive use of an object" one.
   */
  createCommand(): EngineCommand {
    if (this.started) {
      throw new Error(
        'EngineSession.createCommand() called after start(): once the ' +
          "engine runs, run()'s future holds the engine object's wasm " +
          'borrow and no further command handles can be created. Create ' +
          'every handle up front — before start(), or via the watchSlots ' +
          'option for watches.',
      );
    }
    return this.engine.engineCommand();
  }

  /**
   * Registers `callback` for block change notifications. Each watch
   * permanently consumes one command handle (`createWatch` never
   * resolves): before {@link start} a fresh handle is minted, after it
   * one of the pre-created `watchSlots` handles is used — when those
   * run out this throws instead of tripping the wasm borrow guard.
   */
  watch(callback: (notification: BlockNotification) => void): void {
    const handle = this.started
      ? this.watchReserve.pop()
      : this.engine.engineCommand();
    if (!handle) {
      throw new Error(
        'EngineSession.watch() has no command handle left: watches ' +
          'created after start() draw from the pre-created watchSlots ' +
          `pool (size ${String(this.watchReserve.length)} now). Raise the ` +
          'watchSlots option, or create watches before start().',
      );
    }
    // createWatch resolves only on failure to reach the engine; a
    // healthy watch keeps the promise pending forever.
    handle.createWatch(callback).catch((err: unknown) => {
      console.error('logic-mesh: block watch failed:', err);
    });
  }

  /**
   * Resets the engine — terminates all blocks and links, and detaches,
   * stops and unregisters the engine-managed connectors. Sent through
   * the internal control handle; resolves when the reset is enqueued
   * (engine messages are FIFO, so anything awaited on any handle
   * afterwards observes the reset done).
   *
   * Throws after {@link stop} — there is nothing left to reset, and
   * the request would otherwise be swallowed silently by the ended
   * message loop. (A reset issued concurrently with stop() is fine:
   * both go through the serialized control handle, reset first.)
   */
  reset(): Promise<void> {
    if (this.stopDone) {
      throw new Error(
        'EngineSession.reset() called after stop(): the engine has shut ' +
          'down and no longer services commands. Create a new session to ' +
          'run again.',
      );
    }
    return this.enqueueControl(() => this.control.resetEngine());
  }

  /**
   * Shuts the engine down and resolves once `run()`'s future has
   * completed. A no-op before {@link start}. The session is spent
   * afterwards — commands are no longer serviced; create a new session
   * to run again.
   */
  stop(): Promise<void> {
    const runDone = this.runDone;
    if (!runDone) return Promise.resolve();
    // Idempotent: a second Shutdown would be sent into the already
    // closed message channel and reject, so every caller shares the
    // first stop's promise.
    this.stopDone ??= this.enqueueControl(async () => {
      await this.control.stopEngine();
      await runDone;
    });
    return this.stopDone;
  }

  private enqueueControl<T>(op: () => Promise<T>): Promise<T> {
    const next = this.controlChain.catch(() => {}).then(op);
    this.controlChain = next;
    return next;
  }
}

/**
 * Creates an {@link EngineSession} without starting it: the engine and
 * every command handle exist, `run()` has not been kicked off. For
 * hosts that must separate handle creation (module load) from engine
 * start (mount) — call {@link EngineSession.start} when ready. Most
 * embedders want {@link startEngine} instead.
 */
export function createEngineSession(
  options: EngineSessionOptions = {},
): EngineSession {
  return new EngineSession(options);
}

/**
 * Creates the engine (or adopts a not-yet-run one), pre-creates the
 * command handles, kicks off `run()` un-awaited, and returns the
 * ready-to-use {@link EngineSession} — the whole ordering-sensitive
 * start sequence in one call. See the {@link EngineSession} docs for
 * why the order matters.
 *
 * ```ts
 * const { command } = startEngine();
 * const id = await command.addBlock('SineWave');
 * ```
 */
export function startEngine(options: EngineSessionOptions = {}): EngineSession {
  const session = new EngineSession(options);
  session.start();
  return session;
}
