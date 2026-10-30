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
   * The engine's scan interval in milliseconds — how long a block that
   * runs on every cycle waits between runs — forwarded to `initEngine`
   * when the session creates the engine itself. Ignored when an
   * `engine` is supplied.
   */
  scanInterval?: number;
}

/**
 * A watch {@link EngineSession.watch} opened. Close it with
 * {@link close}, or let `await using` close it.
 */
export interface WatchSubscription extends AsyncDisposable {
  /** The watch's id, as `EngineCommand.createWatch` resolved with it. */
  readonly id: number;

  /**
   * Closes the watch, resolving with whether the engine still had it
   * open — `false` once the session has stopped. The callback is not
   * called from the moment this is called. Idempotent: later calls
   * share the first call's promise.
   */
  close(): Promise<boolean>;
}

/** The {@link WatchSubscription} {@link EngineSession.watch} returns. */
class Subscription implements WatchSubscription {
  private closing: Promise<boolean> | undefined;

  constructor(
    readonly id: number,
    private readonly closeWatch: () => Promise<boolean>,
  ) {}

  close(): Promise<boolean> {
    this.closing ??= this.closeWatch();
    return this.closing;
  }

  async [Symbol.asyncDispose](): Promise<void> {
    await this.close();
  }
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
export class EngineSession implements AsyncDisposable {
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

  /** Whether `run()`'s future has completed: the engine answers
   * nothing from then on. */
  private ended = false;

  constructor(options: EngineSessionOptions = {}) {
    this.engine = options.engine ?? initEngine(options.scanInterval);
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
   * Throws after {@link stop}: a session runs its engine once. Once
   * the engine has stopped, the session answers {@link watch},
   * {@link unwatch}, {@link reset} and {@link stop} without asking the
   * engine, and a restart under it would contradict that.
   */
  start(): void {
    if (this.stopDone) {
      throw new Error(
        'EngineSession.start() called after stop(): a session runs its ' +
          'engine once. Create a new session to run again.',
      );
    }
    if (this.runDone) return;
    const run = this.engine.run();
    this.runDone = run;
    // Handling a rejection too keeps a shutdown-time one from surfacing
    // as an unhandled rejection when nobody ever calls stop(); stop()
    // still sees it.
    const end = (): void => {
      this.ended = true;
    };
    run.then(end, end);
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
   * the watch once the engine opened it — before or after
   * {@link start}: before, the watch opens as the engine starts. Close
   * the watch to stop the notifications:
   *
   * ```ts
   * await using watch = await session.watch((n) => console.log(n));
   * // ...
   * // Closed when the scope ends, or earlier with `watch.close()`.
   * ```
   *
   * The watch may outlive the session: once the session has stopped,
   * closing it resolves `false` at once. Rejects if the session stops
   * before the engine opened the watch.
   */
  async watch(
    callback: (notification: BlockNotification) => void,
  ): Promise<WatchSubscription> {
    // Closing turns delivery off at once, before the engine answers.
    let open = true;
    const id = await this.unlessStopped(
      () =>
        this.command.createWatch((notification) => {
          if (open) callback(notification);
        }),
      () => {
        throw new Error(
          'EngineSession.watch() called after stop(): the engine has shut ' +
            'down and opens no watches. Create a new session to run again.',
        );
      },
    );
    return new Subscription(id, () => {
      open = false;
      return this.unwatch(id);
    });
  }

  /**
   * Closes the watch with id `watchId` — one `EngineCommand.createWatch`
   * opened, or a {@link WatchSubscription.id} — resolving with whether
   * it was open. Once the session has stopped, resolves `false`: the
   * engine is gone, and its watches with it.
   */
  unwatch(watchId: number): Promise<boolean> {
    return this.unlessStopped(
      () => this.command.removeWatch(watchId),
      () => false,
    );
  }

  /**
   * Resets the engine — terminates all blocks and links, and detaches,
   * stops and unregisters the engine-managed connectors. Resolves once
   * the reset is done, so a command issued afterwards sees the engine
   * empty.
   *
   * Rejects once the engine has stopped — through {@link stop}, or a
   * `stopEngine` sent on a command handle — or if it stops before
   * answering: there is nothing left to reset, and the request would
   * otherwise wait on the ended message loop forever. (A reset issued
   * concurrently with stop() is fine: both go through the control
   * chain, reset first.)
   */
  reset(): Promise<void> {
    const stopped = (): never => {
      throw new Error(
        'EngineSession.reset() called after stop(): the engine has shut ' +
          'down and no longer services commands. Create a new session to ' +
          'run again.',
      );
    };
    if (this.stopDone) return Promise.resolve().then(stopped);
    return this.enqueueControl(() =>
      this.unlessStopped(() => this.control.resetEngine(), stopped),
    );
  }

  /**
   * Shuts the engine down and resolves once `run()`'s future has
   * completed. A no-op before {@link start}. The session is spent
   * afterwards — commands are no longer serviced; create a new session
   * to run again.
   *
   * If the engine has already stopped — through a `stopEngine` sent on
   * a command handle — or stops while this waits, it resolves once
   * `run()`'s future has completed, without waiting for an answer the
   * stopped engine will never send.
   */
  stop(): Promise<void> {
    const runDone = this.runDone;
    if (!runDone) return Promise.resolve();
    // Idempotent: a second Shutdown would wait on the ended message
    // loop forever, so every caller shares the first stop's promise.
    this.stopDone ??= this.enqueueControl(async () => {
      await this.unlessStopped(
        () => this.control.stopEngine(),
        () => undefined,
      );
      await runDone;
    });
    return this.stopDone;
  }

  /**
   * {@link stop}s the session, so that `await using` stops it when the
   * scope ends:
   *
   * ```ts
   * await using session = startEngine();
   * ```
   */
  async [Symbol.asyncDispose](): Promise<void> {
    await this.stop();
  }

  /**
   * Sends a request with `send` and settles as it does — or with
   * `stopped()` once the engine has stopped without answering it, or
   * at once, without sending, if it already has. A stopped engine
   * answers nothing, and its channel keeps a request it will never
   * read, so the request would otherwise wait forever.
   */
  private unlessStopped<T>(
    send: () => Promise<T>,
    stopped: () => T,
  ): Promise<T> {
    if (this.ended) return Promise.resolve().then(stopped);
    const request = send();
    const runDone = this.runDone;
    // Before start(), the request waits for the engine to start.
    if (!runDone) return request;
    return Promise.race([request, runDone.then(stopped, stopped)]);
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
