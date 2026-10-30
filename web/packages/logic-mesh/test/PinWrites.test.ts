// Engine pin writes against the real wasm engine in Node: a write to an
// input runs its block, whether or not the input is linked. Each test
// runs its own engine session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import type { BlockDesc, BlockNotification, EngineSession } from '../src/index';
import { createEngineSession, startEngine } from '../src/index';

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

/** The latest value a notification reported for the block's `out` pin. */
function lastOut(notes: BlockNotification[], id: string): unknown {
  return notes
    .filter((n) => n.id === id)
    .flatMap((n) => n.changes)
    .filter((c) => c.name === 'out')
    .at(-1)?.value;
}

describe('engine pin writes', () => {
  it('recompute a native block with no links', async () => {
    const session = startEngine({ sleepDuration: 10 });
    sessions.push(session);
    const notes: BlockNotification[] = [];
    session.watch((n) => notes.push(n));

    const id = await session.command.addBlock('Add');
    await session.command.writeBlockInput(id, 'in0', 5);
    await until(() => (lastOut(notes, id) === 5 ? true : undefined), 'out = 5');

    await session.command.writeBlockInput(id, 'in1', 2);
    await until(() => (lastOut(notes, id) === 7 ? true : undefined), 'out = 7');
  });

  it("resolve with the pin's previous value", async () => {
    const session = startEngine({ sleepDuration: 10 });
    sessions.push(session);
    const { command } = session;

    const id = await command.addBlock('Add');
    // Held, the block cannot recompute `out` between the writes.
    await command.pauseExecution();
    expect(await command.writeBlockOutput(id, 'out', 5)).toBeUndefined();
    expect(await command.writeBlockOutput(id, 'out', 7)).toBe(5);
    expect(await command.writeBlockInput(id, 'in0', 1)).toBeUndefined();
    expect(await command.writeBlockInput(id, 'in0', 2)).toBe(1);
    await command.resumeExecution();
  });

  it('run a registered JS block whose inputs are not linked', async () => {
    const session = createEngineSession({ sleepDuration: 10 });
    sessions.push(session);
    const desc: BlockDesc = {
      name: 'DoublesUnlinked',
      dis: 'Doubles',
      lib: 'pin-write-test',
      ver: '0.0.1',
      category: 'Test',
      doc: 'Doubles `in`.',
      implementation: 'external',
      inputs: [{ name: 'in', kind: 'number' }],
      outputs: [{ name: 'out', kind: 'number' }],
    };
    session.engine.registerBlock(desc, () => async (inputs: unknown[]) => {
      const value = inputs[0];
      return typeof value === 'number' ? [value * 2] : undefined;
    });
    session.start();
    const notes: BlockNotification[] = [];
    session.watch((n) => notes.push(n));

    const id = await session.command.addBlock(desc.name, undefined, desc.lib);
    await session.command.writeBlockInput(id, 'in', 21);

    const out = await until(() => lastOut(notes, id), 'the doubled value');
    expect(out).toBe(42);
  });
});
