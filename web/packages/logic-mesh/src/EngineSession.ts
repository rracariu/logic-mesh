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
 * The session bakes the correct order in: its command handles are
 * created up front, {@link start} kicks `run()` off without awaiting
 * it, and {@link stop} / {@link reset} travel through a dedicated
 * control handle so they never touch the engine object.
 *
 * A command handle takes any number of calls at once: each call's
 * promise settles with the engine's answer to that call. Every handle
 * feeds the same engine, which handles requests in the order they
 * arrive. A failed command rejects with a `LogicMeshError` (see
 * `isLogicMeshError`).
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
   * A handle dedicated to connector attachment, kept for callers that
   * want their attach flows on a handle of their own.
   */
  readonly connectorCommand: EngineCommand;

  /** Stop/reset handle — see the class docs. */
  private readonly control: EngineCommand;

  /** Orders control operations: a reset issued before a stop runs
   * first. */
  private controlChain: Promise<unknown> = Promise.resolve();

  private runDone: Promise<void> | undefined;

  private stopDone: Promise<void> | undefined;

  constructor(options: EngineSessionOptions = {}) {
    this.engine = options.engine ?? initEngine(options.sleepDuration);
    this.command = this.engine.engineCommand();
    this.connectorCommand = this.engine.engineCommand();
    this.control = this.engine.engineCommand();
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
   * the raw "recursive use of an object" one. The session's handles
   * take overlapping calls, so most hosts need no more.
   */
  createCommand(): EngineCommand {
    if (this.started) {
      throw new Error(
        'EngineSession.createCommand() called after start(): once the ' +
          "engine runs, run()'s future holds the engine object's wasm " +
          'borrow and no further command handles can be created. Create ' +
          'every handle up front, before start().',
      );
    }
    return this.engine.engineCommand();
  }

  /**
   * Registers `callback` for block change notifications, resolving with
   * the watch's id once the engine opened it — before or after
   * {@link start}: before, the watch opens as the engine starts. Pass
   * the id to {@link unwatch} to stop the notifications.
   */
  watch(callback: (notification: BlockNotification) => void): Promise<number> {
    return this.command.createWatch(callback);
  }

  /**
   * Closes the watch {@link watch} resolved with `watchId`, resolving
   * with whether it was open.
   */
  unwatch(watchId: number): Promise<boolean> {
    return this.command.removeWatch(watchId);
  }

  /**
   * Resets the engine — terminates all blocks and links, and detaches,
   * stops and unregisters the engine-managed connectors. Resolves once
   * the reset is done, so a command issued afterwards sees the engine
   * empty.
   *
   * Throws after {@link stop} — there is nothing left to reset, and
   * the request would otherwise wait on the ended message loop
   * forever. (A reset issued concurrently with stop() is fine: both go
   * through the control chain, reset first.)
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
    // Idempotent: a second Shutdown would wait on the ended message
    // loop forever, so every caller shares the first stop's promise.
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
