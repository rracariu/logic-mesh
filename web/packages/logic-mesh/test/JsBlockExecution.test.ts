// How a registered JS block's executor is called and how its result is
// read, against the real wasm engine in Node. Each test runs its own
// engine session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import type { BlockDesc, BlockNotification, EngineSession } from '../src/index';
import { createEngineSession } from '../src/index';

const sessions: EngineSession[] = [];

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

/** A JS block description with one number input and output. */
function jsDesc(name: string): BlockDesc {
  return {
    name,
    dis: name,
    lib: 'js-block-execution-test',
    ver: '0.0.1',
    category: 'Test',
    doc: 'A test block.',
    implementation: 'external',
    inputs: [{ name: 'in', kind: 'number' }],
    outputs: [{ name: 'out', kind: 'number' }],
  };
}

/**
 * A session, not yet started, whose notifications are collected in
 * `notes` once it starts.
 */
function session(): { session: EngineSession; notes: BlockNotification[] } {
  const session = createEngineSession({ scanInterval: 10 });
  sessions.push(session);
  const notes: BlockNotification[] = [];
  void session.watch((n) => notes.push(n));
  return { session, notes };
}

/** The latest value notifications reported for the `out` pin of `id`,
 * or `undefined` while it is Null. */
function lastOut(notes: BlockNotification[], id: string): unknown {
  return (
    notes
      .filter((n) => n.id === id)
      .flatMap((n) => n.changes)
      .filter((c) => c.name === 'out')
      .at(-1)?.value ?? undefined
  );
}

/** The latest fault notification of block `id`. */
function lastFault(
  notes: BlockNotification[],
  id: string,
): BlockNotification | undefined {
  return notes.filter((n) => n.id === id && n.state === 'fault').at(-1);
}

describe('JS block execution', () => {
  // Regression: a synchronous executor's array was awaited as a
  // Promise, and the TypeError from its missing `then` was thrown
  // through the wasm frames.
  it('takes outputs from a synchronous executor', async () => {
    const { session: s, notes } = session();
    const desc = jsDesc('SyncDoubles');
    s.engine.registerBlock(desc, () => (inputs: unknown[]) => {
      const value = inputs[0];
      return typeof value === 'number' ? [value * 2] : undefined;
    });
    s.start();

    const id = await s.command.addBlock(desc.name, undefined, desc.lib);
    await s.command.writeBlockInput(id, 'in', 21);
    await until(
      () => (lastOut(notes, id) === 42 ? true : undefined),
      'out = 42',
    );
    expect(lastFault(notes, id)).toBeUndefined();
  });

  // Regression: a rejection was decoded as a string only, so a thrown
  // Error faulted the block with a serde "invalid type" message, and a
  // synchronous throw with the error's debug form.
  it("faults the block with a thrown Error's message", async () => {
    const { session: s, notes } = session();
    const sync = jsDesc('ThrowsSync');
    const async = jsDesc('ThrowsAsync');
    s.engine.registerBlock(sync, () => () => {
      throw new RangeError('sync kaboom');
    });
    s.engine.registerBlock(async, () => async () => {
      throw new Error('async kaboom');
    });
    s.start();

    for (const [desc, message] of [
      [sync, 'RangeError: sync kaboom'],
      [async, 'Error: async kaboom'],
    ] as const) {
      const id = await s.command.addBlock(desc.name, undefined, desc.lib);
      await s.command.writeBlockInput(id, 'in', 1);
      const fault = await until(
        () => lastFault(notes, id),
        `a fault of ${desc.name}`,
      );
      expect(fault.faultReason).toBe(`JS execute failed: ${message}`);
    }
  });
});
