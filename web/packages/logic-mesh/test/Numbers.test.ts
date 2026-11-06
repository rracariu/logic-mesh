// Numbers the engine hands to JavaScript, against the real wasm engine in
// Node: whole numbers too large for a JavaScript number to hold every
// integer of, and the non-finite ones. Each test runs its own engine
// session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import type { BlockNotification, EngineSession } from '../src/index';
import { startEngine } from '../src/index';

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

/** The values notifications reported for the block's `out` pin. */
function outs(notes: BlockNotification[], id: string): unknown[] {
  return notes
    .filter((n) => n.id === id)
    .flatMap((n) => n.changes)
    .filter((c) => c.name === 'out')
    .map((c) => c.value);
}

describe('numbers handed to JavaScript', () => {
  // libhaystack serializes a whole Number that fits an i64 as one. Past
  // 2^53 − 1 the bindings used to reject it, failing every method that
  // carried it and dropping its notifications. 2^53 is the first such
  // value, and the largest double below 2^63 the last.
  const large = [2 ** 53, 1e16, 1e18, 9223372036854774784, 1e20, 1.5e300];

  it.each(large.flatMap((v) => [v, -v]))(
    'pass %d through inspect, watch, save and load',
    async (value) => {
      const session = startEngine({ scanInterval: 10 });
      sessions.push(session);
      const notes: BlockNotification[] = [];
      session.watch((n) => notes.push(n));
      const { command } = session;

      const id = await command.addBlock('Neg');
      await command.writeBlockInput(id, 'in', value);
      await until(
        () => (outs(notes, id).includes(-value) ? true : undefined),
        `out = ${-value}`,
      );
      expect((await command.inspectBlock(id)).outputs.out.val).toBe(-value);

      const program = await command.getProgram();
      expect(program.blocks[id].inputs.in.value).toBe(value);

      await command.resetEngine();
      await command.loadProgram(program);
      expect((await command.inspectBlock(id)).inputs.in.val).toBe(value);
    },
  );

  it('pass a large number nested in a list', async () => {
    const session = startEngine();
    sessions.push(session);

    expect(await session.command.evalBlock('List', [1e16, 1])).toEqual([
      [1e16, 1],
    ]);
  });

  it('encode the non-finite numbers as Hayson, and load them back', async () => {
    const session = startEngine({ scanInterval: 10 });
    sessions.push(session);
    const { command } = session;

    for (const [value, val] of [
      [Infinity, 'INF'],
      [-Infinity, '-INF'],
      [NaN, 'NaN'],
    ] as const) {
      const id = await command.addBlock('Abs');
      await command.writeBlockInput(id, 'in', value);

      const encoded = { _kind: 'number', val };
      expect((await command.inspectBlock(id)).inputs.in.val).toEqual(encoded);

      const program = await command.getProgram();
      await command.resetEngine();
      await command.loadProgram(program);
      expect((await command.inspectBlock(id)).inputs.in.val).toEqual(encoded);

      await command.resetEngine();
    }
  });
});
