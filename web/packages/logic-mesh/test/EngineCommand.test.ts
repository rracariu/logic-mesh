// The engine commands, what they accept, and the errors the bindings
// throw, against the real wasm engine in Node. Each test runs its own
// engine session, stopped afterwards.
import { runInNewContext } from 'node:vm';
import { afterEach, describe, expect, it } from 'vitest';
import type {
  BlockDesc,
  BlockNotification,
  ConnectorCallback,
  EngineSession,
  JsBlockFactory,
  JsConnector,
  LogicMeshError,
} from '../src/index';
import * as logicMesh from '../src/index';
import {
  createEngineSession,
  initEngine,
  isLogicMeshError,
  registerConnector,
  startEngine,
  unregisterConnector,
} from '../src/index';

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

/** The latest value notifications reported for pin `pin` of block `id`,
 * or `undefined` while it is Null. */
function last(notes: BlockNotification[], id: string, pin = 'out'): unknown {
  return (
    notes
      .filter((n) => n.id === id)
      .flatMap((n) => n.changes)
      .filter((c) => c.name === pin)
      .at(-1)?.value ?? undefined
  );
}

/** Awaits `promise`, expecting it to reject with a `LogicMeshError`. */
async function rejection(promise: Promise<unknown>): Promise<LogicMeshError> {
  const err: unknown = await promise.then(
    () => undefined,
    (err: unknown) => err,
  );
  if (!isLogicMeshError(err)) {
    throw new Error(`expected a LogicMeshError, got ${String(err)}`, {
      cause: err,
    });
  }
  return err;
}

/** Runs `f`, expecting it to throw a `LogicMeshError`. */
function thrown(f: () => unknown): LogicMeshError {
  try {
    f();
  } catch (err) {
    if (isLogicMeshError(err)) return err;
    throw new Error(`expected a LogicMeshError, got ${String(err)}`, {
      cause: err,
    });
  }
  throw new Error('expected a LogicMeshError, but nothing was thrown');
}

/** A JS block description with one number input and output. */
function jsDesc(name: string): BlockDesc {
  return {
    name,
    dis: name,
    lib: 'engine-command-test',
    ver: '0.0.1',
    category: 'Test',
    doc: 'A test block.',
    implementation: 'external',
    inputs: [{ name: 'in', kind: 'number' }],
    outputs: [{ name: 'out', kind: 'number' }],
  };
}

describe('EngineCommand', () => {
  it('evaluates a block by name', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    expect(await command.evalBlock('Add', [1, 2])).toStrictEqual([3]);
    expect(await command.evalBlock('Add', [1, 2], 'core')).toStrictEqual([3]);
  });

  it('inspects a block as plain objects', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const id = await command.addBlock('Add');
    await command.writeBlockInput(id, 'in0', 4);

    const block = await command.inspectBlock(id);
    expect(block).toStrictEqual(JSON.parse(JSON.stringify(block)));
    expect(block.id).toBe(id);
    expect(block.name).toBe('Add');
    expect(block.lib).toBe('core');
    expect(block.state).toBe('running');
    expect(block.inputs.in0).toStrictEqual({
      kind: 'number',
      val: 4,
      isConnected: false,
    });
    expect(block.outputs.out.kind).toBe('number');
  });

  it('creates and removes a link', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const source = await command.addBlock('Add');
    const target = await command.addBlock('Add');

    const link = await command.createLink(source, target, 'out', 'in0');
    expect(link.sourceBlockUuid).toBe(source);
    expect(link.targetBlockPinName).toBe('in0');
    expect(typeof link.id).toBe('string');
    expect((await command.inspectBlock(target)).inputs.in0.isConnected).toBe(
      true,
    );

    expect(await command.removeLink(link.id as string)).toBe(true);
    expect(await command.removeLink(link.id as string)).toBe(false);
    expect((await command.inspectBlock(target)).inputs.in0.isConnected).toBe(
      false,
    );
  });

  it('pauses and resumes block execution', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    const { command } = session;
    const notes: BlockNotification[] = [];
    await session.watch((n) => notes.push(n));
    const id = await command.addBlock('Add');

    await command.pauseExecution();
    await command.writeBlockInput(id, 'in0', 5);
    await new Promise((resolve) => setTimeout(resolve, 100));
    // Requests are answered while paused; the block does not run.
    expect((await command.inspectBlock(id)).inputs.in0.val).toBe(5);
    expect(last(notes, id)).toBeUndefined();

    await command.resumeExecution();
    await until(() => (last(notes, id) === 5 ? true : undefined), 'out = 5');
  });

  it('feeds an ExternalIn block from a raw JS connector', async () => {
    const session = track(startEngine({ scanInterval: 10 }));
    const { command } = session;
    const name = `raw-${crypto.randomUUID()}`;
    let push: ConnectorCallback | undefined;
    let unsubscribed = false;
    const connector: JsConnector = {
      subscribe(address, callback) {
        expect(address).toBe('temperature');
        push = callback;
        return () => {
          unsubscribed = true;
        };
      },
      publish() {},
      request: (_address, value) => value,
    };
    registerConnector(name, connector);
    await command.addConnector(name);

    const notes: BlockNotification[] = [];
    await session.watch((n) => notes.push(n));
    const id = await command.addBlock('ExternalIn');
    await command.writeBlockInput(id, 'connector', name);
    await command.writeBlockInput(id, 'address', 'temperature');

    const callback = await until(() => push, 'the subscription');
    callback(21.5);
    await until(
      () => (last(notes, id) === 21.5 ? true : undefined),
      'the pushed value',
    );

    await command.removeBlock(id);
    await until(() => (unsubscribed ? true : undefined), 'the unsubscribe');
    await command.removeConnector(name);
  });
});

describe('the value encoding', () => {
  // Regression: watch notifications, evalBlock, writeBlock* and JS block
  // and connector calls handed a Dict to JS as a Map and Null as
  // undefined, where inspectBlock and getProgram gave a plain object and
  // null.
  it('is the same from every method and callback', async () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    const echo: BlockDesc = {
      ...jsDesc('Echo'),
      lib: `encoding-${crypto.randomUUID()}`,
      inputs: [
        { name: 'in', kind: 'dict' },
        { name: 'unset', kind: 'number' },
      ],
      outputs: [{ name: 'out', kind: 'dict' }],
    };
    const calls: unknown[][] = [];
    session.engine.registerBlock(echo, () => (inputs) => {
      calls.push(inputs);
      return [inputs[0]];
    });
    session.start();
    const notes: BlockNotification[] = [];
    await session.watch((n) => notes.push(n));
    const { command } = session;
    const dict = { a: 1, nested: { b: 'x' }, list: [{ c: true }] };

    expect(await command.evalBlock('Dict', [['a'], [1]])).toStrictEqual([
      { a: 1 },
    ]);

    const id = await command.addBlock('Dict');
    await command.writeBlockInput(id, 'keys', ['a']);
    await command.writeBlockInput(id, 'values', [1]);
    expect(await until(() => last(notes, id), 'the Dict output')).toEqual({
      a: 1,
    });
    expect((await command.inspectBlock(id)).outputs.out.val).toStrictEqual({
      a: 1,
    });
    const program = await command.getProgram();
    expect(program.blocks[id].outputs?.out.value).toStrictEqual({ a: 1 });

    // Written twice, so the value resolved is the same whether it is
    // the one written or the one it replaced.
    await command.writeBlockOutput(id, 'out', dict);
    expect(await command.writeBlockOutput(id, 'out', dict)).toStrictEqual(dict);
    await command.writeBlockOutput(id, 'out', null);
    expect(await command.writeBlockOutput(id, 'out', null)).toBeNull();
    const cleared = await until(
      () =>
        notes
          .filter((n) => n.id === id)
          .flatMap((n) => n.changes)
          .find((c) => c.name === 'out' && c.value === null),
      'the Null output change',
    );
    expect(cleared.value).toBeNull();
    expect(await command.writeBlockInput(id, 'keys', null)).toStrictEqual([
      'a',
    ]);
    expect(await command.writeBlockInput(id, 'keys', ['b'])).toBeNull();

    const echoId = await command.addBlock(echo.name, undefined, echo.lib);
    await command.writeBlockInput(echoId, 'in', dict);
    expect(
      await until(() => last(notes, echoId), 'the echoed Dict'),
    ).toStrictEqual(dict);
    expect(calls.at(-1)).toStrictEqual([dict, null]);
  });
});

describe('EngineCommand input validation', () => {
  // Regression: a value that is not a Haystack value was written as Null.
  it('rejects a value that is not a Haystack value', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const id = await command.addBlock('Add');

    const err = await rejection(command.writeBlockInput(id, 'in0', () => 1));
    expect(err.code).toBe('InvalidValue');
    expect(err.message).toContain("input 'in0'");
    expect(
      (await rejection(command.writeBlockOutput(id, 'out', Symbol('x')))).code,
    ).toBe('InvalidValue');
    expect((await rejection(command.evalBlock('Add', [1, () => 2]))).code).toBe(
      'InvalidValue',
    );
    // Nothing was written.
    expect((await command.inspectBlock(id)).inputs.in0.val).toBeNull();
  });

  it('writes null and undefined as Null', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const id = await command.addBlock('Add');
    await command.writeBlockInput(id, 'in0', 3);

    expect(await command.writeBlockInput(id, 'in0', null)).toBe(3);
    expect((await command.inspectBlock(id)).inputs.in0.val).toBeNull();
    await command.writeBlockInput(id, 'in0', 3);
    await command.writeBlockInput(id, 'in0', undefined);
    expect((await command.inspectBlock(id)).inputs.in0.val).toBeNull();
  });

  // Regression: serde-wasm-bindgen reads any object that is not
  // iterable through its own properties, so a Date was written as an
  // empty Dict and a class instance as a Dict of its fields. A cyclic
  // value overflowed the stack.
  it('rejects objects the JSON encoding has no use for', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const id = await command.addBlock('Add');
    class Reading {
      constructor(readonly val: number) {}
    }
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;

    const invalid: [unknown, string][] = [
      [new Date(), 'Date objects are not Haystack values'],
      [new Reading(1), 'Reading objects are not Haystack values'],
      [Object(5), 'Number objects are not Haystack values'],
      [{ list: [new Map([['at', new Date()]])] }, 'Date objects'],
      [cyclic, 'levels deep'],
    ];
    for (const [value, detail] of invalid) {
      const err = await rejection(command.writeBlockInput(id, 'in0', value));
      expect(err.code).toBe('InvalidValue');
      expect(err.message).toContain(detail);
    }
    expect(
      (await rejection(command.evalBlock('Add', [1, new Date()]))).code,
    ).toBe('InvalidValue');
    // Nothing was written.
    expect((await command.inspectBlock(id)).inputs.in0.val).toBeNull();

    // Plain objects, from any realm or without a prototype, arrays and
    // Maps are read.
    const valid: unknown[] = [
      { a: 1 },
      Object.assign(Object.create(null) as object, { a: 1 }),
      runInNewContext('({ a: 1 })'),
      new Map([['a', 1]]),
    ];
    for (const value of valid) {
      await command.writeBlockInput(id, 'in0', value);
      expect((await command.inspectBlock(id)).inputs.in0.val).toStrictEqual({
        a: 1,
      });
    }
    await command.writeBlockInput(id, 'in0', [1, { b: 'x' }]);
    expect((await command.inspectBlock(id)).inputs.in0.val).toStrictEqual([
      1,
      { b: 'x' },
    ]);
  });

  it('rejects malformed block ids with InvalidBlockUuid', async () => {
    const { command } = track(startEngine({ scanInterval: 10 }));
    const id = await command.addBlock('Add');

    expect((await rejection(command.addBlock('Add', 'nope'))).code).toBe(
      'InvalidBlockUuid',
    );
    expect(
      (await rejection(command.createLink(id, 'nope', 'out', 'in0'))).code,
    ).toBe('InvalidBlockUuid');
    expect((await rejection(command.removeBlock('nope'))).code).toBe(
      'InvalidBlockUuid',
    );
  });
});

describe('synchronous bindings', () => {
  // Regression: these threw bare strings.
  it('throw LogicMeshErrors with codes', () => {
    const engine = initEngine(10);

    expect(
      thrown(() =>
        engine.registerBlock({
          ...jsDesc('BadKind'),
          inputs: [{ name: 'in', kind: 'numbr' as 'number' }],
        }),
      ).code,
    ).toBe('UnknownPinKind');
    expect(
      thrown(() =>
        engine.registerBlock({
          ...jsDesc('BadRun'),
          runCondition: 'sometimes' as 'always',
        }),
      ).code,
    ).toBe('UnknownRunCondition');
    expect(
      thrown(() =>
        engine.registerBlock({
          ...jsDesc('BadImpl'),
          implementation: 'native',
        }),
      ).code,
    ).toBe('BlockNotExternal');
    expect(
      thrown(() => engine.registerBlock(42 as unknown as BlockDesc)).code,
    ).toBe('InvalidArgument');

    engine.registerBlock(jsDesc('Twice'));
    expect(thrown(() => engine.registerBlock(jsDesc('Twice'))).code).toBe(
      'BlockAlreadyRegistered',
    );

    const name = `bad-${crypto.randomUUID()}`;
    expect(
      thrown(() =>
        registerConnector(name, { publish() {} } as unknown as JsConnector),
      ).code,
    ).toBe('ConnectorMethodMissing');
    const ok: JsConnector = {
      subscribe() {},
      publish() {},
      request: () => null,
    };
    registerConnector(name, ok);
    expect(thrown(() => registerConnector(name, ok)).code).toBe(
      'ConnectorAlreadyRegistered',
    );
    unregisterConnector(name);
  });

  // Regression: a negative scan interval wrapped around to a huge u32,
  // and the constructor took a bigint where initEngine took a number.
  it('take the scan interval as a number of milliseconds', () => {
    expect(thrown(() => initEngine(-1)).code).toBe('InvalidArgument');
    expect(thrown(() => initEngine(Number.NaN)).code).toBe('InvalidArgument');
    expect(
      thrown(() => new logicMesh.BlocksEngine(Number.POSITIVE_INFINITY)).code,
    ).toBe('InvalidArgument');
    expect(() => new logicMesh.BlocksEngine(25)).not.toThrow();
    expect(() => initEngine(0)).not.toThrow();
  });
});

describe('registerBlock', () => {
  /** {@link jsDesc} in a library of its own. */
  function freshDesc(name: string): BlockDesc {
    return { ...jsDesc(name), lib: `register-block-${crypto.randomUUID()}` };
  }

  // Regression: a block registered without a factory, documented as a
  // no-op, was rejected with ExternalLibraryNotFound when added or
  // evaluated, unless another block of its library had a factory.
  it('registers a block without a factory as a no-op', async () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    const left = freshDesc('NoFactory');
    const nulled = freshDesc('NullFactory');
    session.engine.registerBlock(left);
    session.engine.registerBlock(nulled, null);
    session.start();
    const { command } = session;

    for (const desc of [left, nulled]) {
      const id = await command.addBlock(desc.name, undefined, desc.lib);
      await command.writeBlockInput(id, 'in', 1);
      expect((await command.inspectBlock(id)).state).toBe('running');
      expect(await command.evalBlock(desc.name, [1], desc.lib)).toStrictEqual([
        null,
      ]);
    }
  });

  // Regression: a factory that is not a function was kept unchecked
  // and took the block's name, and adding the block then rejected with
  // a dump of the value and the wasm-bindgen glue.
  it('rejects a factory that is not a function, registering nothing', () => {
    const engine = initEngine(10);
    const desc = freshDesc('NotAFunction');

    const err = thrown(() =>
      engine.registerBlock(desc, 42 as unknown as JsBlockFactory),
    );
    expect(err.code).toBe('InvalidArgument');
    expect(err.message).toContain('got number');
    expect(engine.registerBlock(desc, () => () => [1])).toBe('NotAFunction');
  });

  it('reports a factory that fails with what it threw or returned', async () => {
    const session = track(createEngineSession({ scanInterval: 10 }));
    const throws = freshDesc('FactoryThrows');
    const returns = freshDesc('FactoryReturns');
    session.engine.registerBlock(throws, () => {
      throw new TypeError('no executor');
    });
    session.engine.registerBlock(
      returns,
      (() => 42) as unknown as JsBlockFactory,
    );
    session.start();
    const { command } = session;

    const threw = await rejection(
      command.addBlock(throws.name, undefined, throws.lib),
    );
    expect(threw.code).toBe('ExternalFactoryCallFailed');
    expect(threw.message).toBe(
      "External block 'FactoryThrows' factory function failed: TypeError: no executor",
    );
    const returned = await rejection(
      command.evalBlock(returns.name, [1], returns.lib),
    );
    expect(returned.code).toBe('ExternalFactoryReturnedNonFunction');
    expect(returned.message).toBe(
      "External block 'FactoryReturns' factory did not return a function: it returned a value of type number",
    );
  });
});

describe('scan interval', () => {
  // Regression: the interval reached setTimeout cast to an i32, which
  // wrapped 2^31 ms to a negative delay, run at once, and 2^32 + 10 ms
  // to 10 ms, so a block that runs on every cycle spun.
  it('is waited out when longer than setTimeout takes', async () => {
    const intervals = [10, 2 ** 31, 2 ** 32 + 10, Number.MAX_SAFE_INTEGER];
    const runs = intervals.map(() => 0);
    await Promise.all(
      intervals.map((scanInterval, index) => {
        const session = track(createEngineSession({ scanInterval }));
        const desc: BlockDesc = {
          name: `ScanEvery${index}`,
          dis: `Scan every ${scanInterval} ms`,
          lib: 'scan-interval-test',
          ver: '0.0.1',
          category: 'Test',
          doc: 'Counts its runs.',
          implementation: 'external',
          inputs: [],
          outputs: [{ name: 'out', kind: 'number' }],
          runCondition: 'always',
        };
        session.engine.registerBlock(desc, () => () => {
          runs[index]++;
          return undefined;
        });
        session.start();
        return session.command.addBlock(desc.name, undefined, desc.lib);
      }),
    );

    await new Promise((resolve) => setTimeout(resolve, 500));
    // The block runs on every cycle of the 10 ms engine...
    expect(runs[0]).toBeGreaterThan(5);
    // ...and waits the longer intervals out.
    expect(runs.slice(1)).toStrictEqual([0, 0, 0]);
  });
});
