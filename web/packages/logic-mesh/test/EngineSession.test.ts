// These tests run against the real wasm engine in Node: the vitest
// alias maps the sources' `./logic_mesh.js` import onto the built
// module in `dist/` (build first with `npm run build:dev`). Each test
// creates its own engine; sessions are stopped afterwards so no engine
// loop outlives its test.
import { afterEach, describe, expect, it } from 'vitest';
import type {
  BlockDesc,
  BlockNotification,
  EngineSession,
  JsConnector,
  LogicMeshError,
} from '../src/index';
import {
  connectorRegistered,
  createEngineSession,
  initEngine,
  isLogicMeshError,
  registerConnector,
  startEngine,
  unregisterConnector,
} from '../src/index';

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

const sessions: EngineSession[] = [];

function track(session: EngineSession): EngineSession {
  sessions.push(session);
  return session;
}

afterEach(async () => {
  await Promise.all(sessions.splice(0).map((s) => s.stop()));
});

/** Polls `get` until it returns a value, or fails after `timeoutMs`. */
async function until<T>(
  get: () => T | undefined,
  what: string,
  timeoutMs = 3000,
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = get();
    if (value !== undefined) return value;
    if (Date.now() > deadline) throw new Error(`timed out waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

describe('startEngine', () => {
  it('returns handles that keep working after run() has been polled', async () => {
    const session = track(startEngine({ scanInterval: 10 }));

    // Two settled microtask hops guarantee run()'s future has been
    // polled — from here on the engine object's wasm borrow is held.
    await Promise.resolve();
    await Promise.resolve();

    // The exact failure the wrapper prevents: minting a handle now
    // trips the wasm borrow guard...
    expect(() => session.engine.engineCommand()).toThrow(/recursive use/);

    // ...while the session's pre-created handles resolve normally.
    const id = await session.command.addBlock('SineWave');
    expect(id).toMatch(UUID_RE);
    expect(await session.connectorCommand.listConnectors()).toEqual([]);
  });

  it('createCommand after start throws a descriptive error, not the wasm one', () => {
    const session = track(startEngine({ scanInterval: 10 }));
    expect(() => session.createCommand()).toThrow(/before start\(\)/);
  });

  it('delivers block notifications through a watch opened after start', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    await Promise.resolve();
    await Promise.resolve();

    const notes: BlockNotification[] = [];
    const watchId = await session.watch((n) => notes.push(n));
    expect(typeof watchId).toBe('number');

    const id = await session.command.addBlock('SineWave');
    const note = await until(
      () => notes.find((n) => n.id === id),
      'a SineWave notification',
    );
    expect(note.state).toBe('running');
  });

  // Regression: each watch used to hold a command handle forever, so
  // a session ran out of watches after its pre-created slots.
  it('opens any number of watches, and unwatch stops one', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    const kept: BlockNotification[] = [];
    const closed: BlockNotification[] = [];
    await session.watch((n) => kept.push(n));
    const closedId = await session.watch((n) => closed.push(n));
    await session.watch(() => {});

    expect(await session.unwatch(closedId)).toBe(true);
    expect(await session.unwatch(closedId)).toBe(false);

    const id = await session.command.addBlock('SineWave');
    await until(() => kept.find((n) => n.id === id), 'a SineWave notification');
    expect(closed.find((n) => n.id === id)).toBeUndefined();
  });
});

/**
 * Registers, under a unique name, a JS connector whose `stop()` takes a
 * while before it records that it finished — so a test can tell an
 * answer sent once the stop is done from one sent while it is under
 * way.
 */
function registerSlowStoppingConnector(): {
  name: string;
  stopped: () => boolean;
} {
  const name = `slow-stop-${crypto.randomUUID()}`;
  let stopped = false;
  const connector: JsConnector = {
    subscribe: () => {},
    publish: () => {},
    request: (_address, value) => value,
    stop: async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
      stopped = true;
    },
  };
  registerConnector(name, connector);
  return { name, stopped: () => stopped };
}

/** Awaits `promise`, expecting it to reject with a `LogicMeshError`. */
async function rejection(promise: Promise<unknown>): Promise<LogicMeshError> {
  const err: unknown = await promise.then(
    () => undefined,
    (err: unknown) => err,
  );
  if (!isLogicMeshError(err)) {
    throw new Error(`expected a LogicMeshError, got ${String(err)}`);
  }
  return err;
}

describe('EngineCommand', () => {
  // Regression: rejections were bare strings, so callers could only
  // tell failures apart by their wording.
  it('rejects with LogicMeshErrors carrying a stable code', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const unknown = '00000000-0000-4000-8000-0000000000ff';

    const missing = await rejection(command.inspectBlock(unknown));
    expect(missing).toBeInstanceOf(Error);
    expect(missing.name).toBe('LogicMeshError');
    expect(missing.code).toBe('BlockInstanceNotFound');
    expect(missing.message).toContain(unknown);

    expect((await rejection(command.addBlock('NoSuchBlock'))).code).toBe(
      'BlockNotRegistered',
    );
    expect((await rejection(command.removeLink('link-1'))).code).toBe(
      'InvalidLinkUuid',
    );
    expect((await rejection(command.removeConnector('nope'))).code).toBe(
      'ConnectorNotManaged',
    );
    expect((await rejection(command.removeWatch(-1))).code).toBe(
      'InvalidArgument',
    );
  });

  // Regression: an unparseable block id silently became the nil UUID.
  it('rejects an unparseable block id instead of using the nil UUID', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const err = await rejection(command.inspectBlock('not-a-uuid'));
    expect(err.code).toBe('InvalidBlockUuid');
    expect(err.message).toContain('not-a-uuid');
  });

  // Regression: replies carried no request id, so overlapping calls on
  // one handle tripped the wasm borrow guard and crashed the process.
  it('answers overlapping calls on one handle each with its own result', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const ids = await Promise.all(
      Array.from({ length: 8 }, () => command.addBlock('Add')),
    );
    expect(new Set(ids).size).toBe(8);

    const inspected = (await Promise.all(
      ids.map((id) => command.inspectBlock(id)),
    )) as { id: string }[];
    expect(inspected.map((block) => block.id)).toStrictEqual(ids);
  });

  // Regression: control commands resolved once enqueued, not once
  // done. The connector is checked before any other command: the
  // engine handles commands in order, so a later one would find the
  // reset done even after an early answer.
  it('resolves resetEngine once its connectors stopped and the engine is empty', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    const { name, stopped } = registerSlowStoppingConnector();
    await session.command.addConnector(name);
    const id = await session.command.addBlock('Add');

    await session.command.resetEngine();
    expect(stopped()).toBe(true);
    expect(connectorRegistered(name)).toBe(false);
    expect(
      (await rejection(session.connectorCommand.inspectBlock(id))).code,
    ).toBe('BlockInstanceNotFound');
  });

  it('resolves stopEngine once its connectors stopped', async () => {
    // A bare engine rather than a session, whose stop() would also
    // wait for run() to end — after the connectors stopped either way.
    const engine = initEngine(10);
    const command = engine.engineCommand();
    const run = engine.run();
    const { name, stopped } = registerSlowStoppingConnector();
    await command.addConnector(name);

    await command.stopEngine();
    expect(stopped()).toBe(true);
    await run;
    unregisterConnector(name);
  });
});

describe('createEngineSession', () => {
  it('defers start; handles minted before start work after it', async () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    expect(session.started).toBe(false);

    // Pre-start setup on the engine object is still allowed.
    expect(session.engine.listBlocks().length).toBeGreaterThan(0);
    const extra = session.createCommand();

    session.start();
    expect(session.started).toBe(true);

    const id = await extra.addBlock('SineWave');
    expect(id).toMatch(UUID_RE);
  });
});

describe('registerBlock / listBlocks round-trip', () => {
  // The block registry is process-wide like the connector registry, so
  // the names here are unique to this suite.
  const desc = {
    name: 'RoundTripAlways',
    dis: 'Round-trip Always',
    lib: 'roundtrip-test',
    ver: '0.0.1',
    category: 'Test',
    doc: 'Runs every cycle.',
    implementation: 'external',
    inputs: [{ name: 'in', kind: 'number' }],
    outputs: [{ name: 'out', kind: 'number' }],
    runCondition: 'always',
  } satisfies BlockDesc;

  it("lists runCondition 'always' as 'always', and the listed desc re-registers", () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    session.engine.registerBlock(desc);

    const listed = (session.engine.listBlocks() as BlockDesc[]).find(
      (b) => b.name === 'RoundTripAlways',
    );
    // The Display copy-paste bug used to render these as
    // 'native'/'external' instead of 'change'/'always'.
    expect(listed?.runCondition).toBe('always');
    expect(listed?.implementation).toBe('external');

    // The listed desc is itself a valid registration payload (under a
    // fresh name — the registry rejects duplicates), proving the two
    // conversions actually round-trip.
    expect(() =>
      session.engine.registerBlock({ ...listed, name: 'RoundTripAlways2' }),
    ).not.toThrow();
  });

  it('rejects a wrong implementation or run condition with a field-naming error', () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    // registerBlock only ever registers JS blocks, which are external
    // by definition — 'native' is a caller mistake, not a choice.
    expect(() =>
      session.engine.registerBlock({
        ...desc,
        name: 'RoundTripNative',
        implementation: 'native',
      }),
    ).toThrow(/implementation.*external/);
    expect(() =>
      session.engine.registerBlock({
        ...desc,
        name: 'RoundTripSometimes',
        runCondition: 'sometimes',
      }),
    ).toThrow(/runCondition.*sometimes/);
  });
});

describe('EngineSession.stop', () => {
  it("resolves run()'s future and is idempotent", async () => {
    const session = startEngine({ scanInterval: 10 });
    const id = await session.command.addBlock('SineWave');
    expect(id).toMatch(UUID_RE);

    await session.stop();
    // A second stop must not hang on the already-dead message loop.
    await session.stop();
  });

  it('is a no-op on a session that was never started', async () => {
    const session = createEngineSession({ scanInterval: 10 });
    await session.stop();
    expect(session.started).toBe(false);
  });
});

describe('EngineSession lifecycle edges', () => {
  it('start() is idempotent', async () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    session.start();
    // A second start() must be a no-op, not a second run() — that
    // would trip the engine object's wasm borrow guard.
    session.start();
    const id = await session.command.addBlock('SineWave');
    expect(id).toMatch(UUID_RE);
  });

  it('adopts a caller-created engine that has not run yet', async () => {
    const engine = initEngine(10);
    const session = track(startEngine({ engine }));
    expect(session.engine).toBe(engine);
    const id = await session.command.addBlock('SineWave');
    expect(id).toMatch(UUID_RE);
  });

  it('runs a same-tick reset() and stop() in order: reset, then stop', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    await session.command.addBlock('SineWave');
    // A stop issued first would leave the reset waiting on the ended
    // message loop forever.
    const reset = session.reset();
    const stop = session.stop();
    await Promise.all([reset, stop]);
    expect(session.started).toBe(true);
  });

  it('start() after stop() throws instead of arming a dead session', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    await session.stop();
    // Without the guard the session would look started while every
    // command hangs forever against the ended message loop.
    expect(() => session.start()).toThrow(/after stop\(\)/);
  });

  it('reset() after stop() throws instead of silently no-opping', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    await session.stop();
    expect(() => session.reset()).toThrow(/after stop\(\)/);
  });
});
