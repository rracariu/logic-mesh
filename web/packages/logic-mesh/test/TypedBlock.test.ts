// Tests for TypedBlock deliberately bypass the TS type checker in
// several places: they pass wrong-shape arrays to `executeImpl`, mock
// the `BlocksEngine`, and intentionally return mistyped tuples to
// exercise runtime validation guards. Disabling `no-explicit-any`
// here lets those negative-path assertions stay readable.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import type { BlockNotification, EngineSession } from '../src/index';
import { createEngineSession } from '../src/index';
import { defineBlock } from '../src/TypedBlock';

const desc = {
  name: 'TestBlock',
  dis: 'Test Block',
  lib: 'test',
  ver: '0.0.1',
  category: 'Test',
  doc: 'A test block',
};

describe('defineBlock', () => {
  it('populates desc, inputs, and outputs', () => {
    const Block = defineBlock({
      desc,
      inputs: [['x', z.number()]] as const,
      outputs: [['y', z.string()]] as const,
    });
    const b = new Block();

    expect(b.desc.name).toBe('TestBlock');
    expect(b.desc.implementation).toBe('external');
    expect(b.inputs).toEqual(['x']);
    expect(b.outputs).toEqual(['y']);
    expect(b.desc.inputs).toEqual([{ name: 'x', kind: 'number' }]);
    expect(b.desc.outputs).toEqual([{ name: 'y', kind: 'str' }]);
  });

  it('supports multiple inputs and outputs', () => {
    const Block = defineBlock({
      desc,
      inputs: [
        ['a', z.string()],
        ['b', z.boolean()],
      ] as const,
      outputs: [['result', z.number()]] as const,
    });
    const b = new Block();

    expect(b.inputs).toEqual(['a', 'b']);
    expect(b.desc.inputs).toEqual([
      { name: 'a', kind: 'str' },
      { name: 'b', kind: 'bool' },
    ]);
  });

  it('registers with the engine without an executor when execute is absent', () => {
    const Block = defineBlock({
      desc,
      inputs: [['x', z.number()]] as const,
      outputs: [['y', z.string()]] as const,
    });
    const engine = { registerBlock: vi.fn() };
    Block.register(engine as any);

    expect(engine.registerBlock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'TestBlock' }),
    );
  });

  it('registers with the engine with an executor when execute is present', () => {
    const Block = defineBlock({
      desc,
      inputs: [['x', z.number()]] as const,
      outputs: [['y', z.string()]] as const,
      execute([x]) {
        return Promise.resolve([String(x)]);
      },
    });
    const engine = { registerBlock: vi.fn() };
    Block.register(engine as any);

    expect(engine.registerBlock).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'TestBlock' }),
      expect.any(Function),
    );
  });

  it('infers execute parameter types from inputs — no annotation needed', async () => {
    // 'n' is inferred as number, 's' as string — no ': [number, string]' required
    const Block = defineBlock({
      desc,
      inputs: [
        ['n', z.number()],
        ['s', z.string()],
      ] as const,
      outputs: [['out', z.boolean()]] as const,
      execute([n, s]) {
        return Promise.resolve([n > 0 && s.length > 0]);
      },
    });

    const b = new Block();
    await expect((b as any).executeImpl([3, 'hi'])).resolves.toEqual([true]);
    await expect((b as any).executeImpl([-1, 'hi'])).resolves.toEqual([false]);
  });
});

describe('zodToKind mapping', () => {
  const cases = [
    { schema: z.boolean(), kind: 'bool' },
    { schema: z.number(), kind: 'number' },
    { schema: z.string(), kind: 'str' },
    { schema: z.enum(['a', 'b']), kind: 'str' },
    { schema: z.array(z.string()), kind: 'list' },
    { schema: z.object({ a: z.string() }), kind: 'dict' },
    { schema: z.unknown(), kind: 'null' },
    { schema: z.optional(z.string()), kind: 'str' },
    { schema: z.string().default('x'), kind: 'str' },
  ] as const;

  for (const { schema, kind } of cases) {
    it(`maps ${schema.constructor.name} to '${kind}'`, () => {
      const Block = defineBlock({
        desc,
        inputs: [['pin', schema]] as const,
        outputs: [['out', z.unknown()]] as const,
      });
      expect(new Block().desc.inputs[0].kind).toBe(kind);
    });
  }
});

describe('TypedBlock.executeImpl', () => {
  // Inline execute — types inferred, no annotation needed
  const TestBase = defineBlock({
    desc,
    inputs: [
      ['x', z.number()],
      ['label', z.string()],
    ] as const,
    outputs: [['result', z.boolean()]] as const,
    execute([x, _label]) {
      return Promise.resolve([x > 0]);
    },
  });

  class TestBlock extends TestBase {
    runImpl(inputs: unknown[]) {
      return this.executeImpl(inputs as any);
    }
  }

  it('throws when input count is wrong', async () => {
    const b = new TestBlock();
    await expect(b.runImpl([1])).rejects.toThrow('Invalid number of inputs');
  });

  it('parses inputs via Zod and forwards them to execute', async () => {
    const b = new TestBlock();
    const executeSpy = vi.spyOn(b, 'execute');
    await b.runImpl([7, 'hello']);
    expect(executeSpy).toHaveBeenCalledWith([7, 'hello']);
  });

  it('returns undefined when execute returns undefined', async () => {
    class UndefBlock extends TestBase {
      override execute(_inputs: [number, string]) {
        return Promise.resolve(undefined);
      }
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    const result = await new UndefBlock().runImpl([1, 'hi']);
    expect(result).toBeUndefined();
  });

  it('throws when output count is wrong', async () => {
    class BadOutputBlock extends TestBase {
      override execute(_inputs: [number, string]) {
        return Promise.resolve([true, 'extra'] as any);
      }
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    await expect(new BadOutputBlock().runImpl([1, 'hi'])).rejects.toThrow(
      'Invalid number of outputs',
    );
  });

  it('returns the execute result', async () => {
    const b = new TestBlock();
    const result = await b.runImpl([5, 'hello']);
    expect(result).toEqual([true]);
  });

  it('throws when input Zod validation fails', async () => {
    const b = new TestBlock();
    await expect(b.runImpl(['not-a-number', 'hello'])).rejects.toThrow();
  });

  it('skips execution — a no-op, not a fault — while a required input is unset', async () => {
    const b = new TestBlock();
    const executeSpy = vi.spyOn(b, 'execute');
    // The engine passes `undefined` for a pin whose value is not set
    // yet (e.g. right after a link is created); a required schema must
    // skip the cycle instead of throwing a permanent-fault ZodError.
    await expect(b.runImpl([undefined, 'hello'])).resolves.toBeUndefined();
    expect(executeSpy).not.toHaveBeenCalled();
  });

  it('materializes a .default(...) for an unset input and executes', async () => {
    const seen: unknown[][] = [];
    const DefaultBase = defineBlock({
      desc,
      inputs: [['x', z.number().default(7)]] as const,
      outputs: [['out', z.number()]] as const,
      execute([x]) {
        seen.push([x]);
        return Promise.resolve([x * 2]);
      },
    });
    class DefaultBlock extends DefaultBase {
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    await expect(new DefaultBlock().runImpl([undefined])).resolves.toEqual([
      14,
    ]);
    expect(seen).toEqual([[7]]);
  });

  it('an .optional() input lets the executor run with undefined', async () => {
    const seen: unknown[][] = [];
    const OptionalBase = defineBlock({
      desc,
      inputs: [['x', z.number().optional()]] as const,
      outputs: [['out', z.boolean()]] as const,
      execute([x]) {
        seen.push([x]);
        return Promise.resolve([x === undefined]);
      },
    });
    class OptionalBlock extends OptionalBase {
      runImpl(inputs: unknown[]) {
        return this.executeImpl(inputs as any);
      }
    }

    await expect(new OptionalBlock().runImpl([undefined])).resolves.toEqual([
      true,
    ]);
    expect(seen).toEqual([[undefined]]);
  });
});

// ---------------------------------------------------------------------------
// Real-engine tests: these run against the built wasm in `dist/` (the
// vitest alias maps the sources' `./logic_mesh.js` import onto it —
// build first with `npm run build:dev`), same infrastructure as
// JsBlockCancellation.test.ts. The mock-based tests above still cover
// descriptor shape; these cover what mocks cannot: the engine's strict
// JsBlockDesc conversion of a defineBlock desc, the
// factory-returns-a-function calling convention, and link-driven
// execution — including the unset-input window that used to fault the
// block permanently.
// ---------------------------------------------------------------------------

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

/** All values notifications reported for the block's `out` pin, in
 * order. Valueless changes are dropped: the block-added notification
 * reports every output pin as a change with no value. */
function outValues(notes: BlockNotification[], id: string): unknown[] {
  return notes
    .filter((n) => n.id === id)
    .flatMap((n) => n.changes)
    .filter((c) => c.name === 'out' && c.value !== undefined)
    .map((c) => c.value);
}

/** The block's fault notifications, if any. */
function faults(notes: BlockNotification[], id: string): BlockNotification[] {
  return notes.filter((n) => n.id === id && n.state === 'fault');
}

/** The block-desc registry is process-wide, so every test registers
 * its block under a fresh name — same idiom as the connector tests. */
let blockSerial = 0;

describe('defineBlock against the real engine', () => {
  it('link-driven values reach execute; the pre-value window is a no-op, not a fault', async () => {
    const session = createEngineSession({ sleepDuration: 10 });
    sessions.push(session);

    const name = `TypedDoubler${blockSerial++}`;
    const Doubler = defineBlock({
      desc: { ...desc, name, lib: 'typed-test' },
      inputs: [['in', z.number()]] as const,
      outputs: [['out', z.number()]] as const,
      execute([v]) {
        return Promise.resolve([v * 2]);
      },
    });
    // Register before start(): registration needs the engine object,
    // whose wasm borrow run()'s future holds from its first poll.
    Doubler.register(session.engine);
    session.start();

    const notes: BlockNotification[] = [];
    session.watch((n) => notes.push(n));

    const typedId = await session.command.addBlock(
      name,
      undefined,
      'typed-test',
    );
    // A reactive block only fires on link traffic — a bare engine pin
    // write does not trigger execution — so the values travel over a
    // real link from an Add block's output. The Add block itself never
    // executes (its inputs stay unconnected, so its input wait never
    // completes); it is just an output pin whose writes propagate
    // through the link — same setup as JsBlockCancellation.test.ts.
    const addId = await session.command.addBlock('Add');
    await session.command.createLink(addId, typedId, 'out', 'in');

    // Pre-value window: the link triggers cycles before any number has
    // flowed, so `executeImpl` sees `undefined` on the required pin.
    // Pre-fix this faulted the block permanently with a ZodError
    // ("expected number, received undefined"); now the cycle is
    // skipped. Give a fault a chance to surface before asserting there
    // is none, and that no output was committed.
    await until(
      () => notes.find((n) => n.id === typedId),
      'a typed-block notification',
    );
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(faults(notes, typedId)).toEqual([]);
    expect(outValues(notes, typedId)).toEqual([]);

    // Push values through the link; each must come out doubled —
    // across several changes, faultless.
    const drives: [number, number][] = [
      [21, 42],
      [32, 64],
      [43, 86],
    ];
    for (const [value, doubled] of drives) {
      await session.command.writeBlockOutput(addId, 'out', value);
      await until(
        () => (outValues(notes, typedId).includes(doubled) ? true : undefined),
        `out=${String(doubled)}`,
      );
    }
    expect(faults(notes, typedId)).toEqual([]);
  });

  it('a .default(...) input lets the executor run without that pin being driven', async () => {
    const session = createEngineSession({ sleepDuration: 10 });
    sessions.push(session);

    const name = `TypedOffset${blockSerial++}`;
    const Offset = defineBlock({
      desc: { ...desc, name, lib: 'typed-test' },
      inputs: [
        ['in', z.number()],
        // Never driven: the engine passes `undefined`, and the default
        // must materialize in the executor's arguments.
        ['k', z.number().default(7)],
      ] as const,
      outputs: [['out', z.number()]] as const,
      execute([v, k]) {
        return Promise.resolve([v + k]);
      },
    });
    Offset.register(session.engine);
    session.start();

    const notes: BlockNotification[] = [];
    session.watch((n) => notes.push(n));

    const typedId = await session.command.addBlock(
      name,
      undefined,
      'typed-test',
    );
    const addId = await session.command.addBlock('Add');
    await session.command.createLink(addId, typedId, 'out', 'in');

    await session.command.writeBlockOutput(addId, 'out', 5);
    await until(
      () => (outValues(notes, typedId).includes(12) ? true : undefined),
      'out=12 (5 + the default 7)',
    );
    expect(faults(notes, typedId)).toEqual([]);
  });
});
