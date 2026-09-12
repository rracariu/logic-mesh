// These tests exercise defineJsBlocks against the real wasm engine in
// Node: JS functions are dispatched through a real connector by real
// `Request` blocks. The connector registry is process-wide, so every
// test uses its own connector name; each test also runs its own engine
// session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import type {
  BlockNotification,
  EngineCommand,
  EngineSession,
} from '../src/index';
import { connectorRegistered, defineJsBlocks, startEngine } from '../src/index';

const sessions: EngineSession[] = [];

afterEach(async () => {
  await Promise.all(sessions.splice(0).map((s) => s.stop()));
});

/**
 * A started engine session (fast 10 ms block cadence) with a watch
 * collecting every block notification.
 */
function session(): { session: EngineSession; notes: BlockNotification[] } {
  const s = startEngine({ sleepDuration: 10 });
  sessions.push(s);
  const notes: BlockNotification[] = [];
  s.watch((n) => notes.push(n));
  return { session: s, notes };
}

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

/** The latest fault notification for the block, if any. */
function lastFault(
  notes: BlockNotification[],
  id: string,
): BlockNotification | undefined {
  return notes.filter((n) => n.id === id && n.state === 'fault').at(-1);
}

describe('defineJsBlocks', () => {
  it('calls a function through a real Request block', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { scale: (value) => (value as number) * 2 },
      { name: 'jsb-scale' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'scale');
    await s.command.writeBlockInput(id, 'in', 21);

    const out = await until(() => lastOut(notes, id), 'the scaled value');
    expect(out).toBe(42);
  });

  it('awaits async functions', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      {
        fetchTemp: async (zone) => {
          await new Promise((resolve) => setTimeout(resolve, 20));
          return `${zone as string}: 21.5`;
        },
      },
      { name: 'jsb-async' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'fetchTemp');
    await s.command.writeBlockInput(id, 'in', 'zone-1');

    const out = await until(() => lastOut(notes, id), 'the async result');
    expect(out).toBe('zone-1: 21.5');
  });

  it('faults the block when the function rejects', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      {
        boom: () => Promise.reject(new Error('kaboom')),
      },
      { name: 'jsb-boom' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'boom');
    await s.command.writeBlockInput(id, 'in', 1);

    const fault = await until(() => lastFault(notes, id), 'a fault');
    expect(fault.faultReason).toContain('Request');
    expect(fault.faultReason).toContain('kaboom');
  });

  it('round-trips multi-input objects as plain JS objects', async () => {
    const { session: s, notes } = session();
    let received: unknown;
    const jsBlocks = defineJsBlocks(
      {
        // The multi-input idiom: one object argument carries the named
        // inputs. The bridge encodes dicts as Maps; the façade hands
        // the function a plain object.
        stats: (value) => {
          received = value;
          const { a, b } = value as { a: number; b: number };
          return { sum: a + b, args: [a, b], nested: { ok: true } };
        },
      },
      { name: 'jsb-dict' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'stats');
    await s.command.writeBlockInput(id, 'in', {
      a: 20,
      b: 22,
      tag: 'sum',
      inner: { deep: 'yes' },
    });

    const out = await until(() => lastOut(notes, id), 'the dict result');
    // The function saw plain objects, nested ones included.
    expect(received).toEqual({
      a: 20,
      b: 22,
      tag: 'sum',
      inner: { deep: 'yes' },
    });
    // On the way out the bridge encodes the dict as a Map again.
    expect(out).toBeInstanceOf(Map);
    const dict = out as Map<string, unknown>;
    expect(dict.get('sum')).toBe(42);
    expect(dict.get('args')).toEqual([20, 22]);
    expect(dict.get('nested')).toBeInstanceOf(Map);
    expect((dict.get('nested') as Map<string, unknown>).get('ok')).toBe(true);
  });

  it('coerces an undefined return to null instead of faulting', async () => {
    const { session: s, notes } = session();
    let calls = 0;
    const jsBlocks = defineJsBlocks(
      {
        sink: () => {
          calls += 1;
        },
      },
      { name: 'jsb-undef' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'sink');
    await s.command.writeBlockInput(id, 'in', 7);

    await until(() => (calls > 0 ? true : undefined), 'the sink call');
    // Give a fault a chance to surface before asserting there is none.
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(lastFault(notes, id)).toBeUndefined();
  });

  it('attach → detach → re-attach works', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { echo: (value) => value },
      { name: 'jsb-cycle' },
    );

    await jsBlocks.attach(s.command);
    expect(await s.command.listConnectors()).toContain('jsb-cycle');
    // Attaching again is a no-op, not a duplicate registration error.
    await jsBlocks.attach(s.command);

    await jsBlocks.detach(s.command);
    expect(await s.command.listConnectors()).not.toContain('jsb-cycle');
    expect(connectorRegistered('jsb-cycle')).toBe(false);
    // Detaching a detached connector is a no-op too.
    await jsBlocks.detach(s.command);

    await jsBlocks.attach(s.command);
    const id = await jsBlocks.addBlock(s.command, 'echo');
    await s.command.writeBlockInput(id, 'in', 'still here');
    const out = await until(() => lastOut(notes, id), 'the echoed value');
    expect(out).toBe('still here');
  });

  it('re-attaches after an engine reset detached it', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { echo: (value) => value },
      { name: 'jsb-reset' },
    );
    await jsBlocks.attach(s.command);

    await s.reset();
    // attach's listConnectors reply is the FIFO barrier proving the
    // reset — which detached and unregistered the connector — is done.
    await jsBlocks.attach(s.command);
    expect(await s.command.listConnectors()).toContain('jsb-reset');

    const id = await jsBlocks.addBlock(s.command, 'echo');
    await s.command.writeBlockInput(id, 'in', 5);
    const out = await until(() => lastOut(notes, id), 'the echoed value');
    expect(out).toBe(5);
  });

  it('rejects unknown function names in addBlock, and faults on unknown addresses', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { known: (value) => value },
      { name: 'jsb-unknown' },
    );
    await jsBlocks.attach(s.command);

    // @ts-expect-error — 'missing' is not a defined function name; the
    // runtime check is exercised on purpose.
    await expect(jsBlocks.addBlock(s.command, 'missing')).rejects.toThrow(
      /no function named 'missing'/,
    );

    // An address pointing at no function (e.g. edited on the pin
    // directly) surfaces as the block's fault, not a silent drop.
    const id = await s.command.addBlock('Request');
    await s.command.writeBlockInput(id, 'connector', 'jsb-unknown');
    await s.command.writeBlockInput(id, 'address', 'missing');
    await s.command.writeBlockInput(id, 'in', 1);
    const fault = await until(() => lastFault(notes, id), 'a fault');
    expect(fault.faultReason).toContain("no function named 'missing'");
  });

  it("the block's timeout pin bounds a hung function", async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { hang: () => new Promise(() => {}) },
      { name: 'jsb-hang' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'hang');
    await s.command.writeBlockInput(id, 'timeout', 50);
    await s.command.writeBlockInput(id, 'in', 1);

    const fault = await until(() => lastFault(notes, id), 'a timeout fault');
    expect(fault.faultReason?.toLowerCase()).toContain('timed out');
  });

  it('refuses a second instance on an owned connector name', async () => {
    const { session: s, notes } = session();
    const owner = defineJsBlocks(
      { echo: (value) => value },
      { name: 'jsb-owner' },
    );
    await owner.attach(s.command);

    // The name is taken: a second instance attaching must fail loudly —
    // an early-return success would leave its blocks silently answered
    // by the owner's functions.
    const squatter = defineJsBlocks(
      { echo: () => 'stolen' },
      { name: 'jsb-owner' },
    );
    await expect(squatter.attach(s.command)).rejects.toThrow(
      /already registered under 'jsb-owner'/,
    );

    // And a non-owner detach must not tear down the owner's attachment
    // or registration.
    await squatter.detach(s.command);
    expect(await s.command.listConnectors()).toContain('jsb-owner');
    expect(connectorRegistered('jsb-owner')).toBe(true);

    const id = await owner.addBlock(s.command, 'echo');
    await s.command.writeBlockInput(id, 'in', 'mine');
    expect(await until(() => lastOut(notes, id), 'the owner answer')).toBe(
      'mine',
    );
  });

  it('does not hijack a name a new owner took after an engine reset', async () => {
    const { session: s, notes } = session();
    const a = defineJsBlocks({ echo: () => 'A' }, { name: 'jsb-handover' });
    await a.attach(s.command);

    // The reset detaches and unregisters A's connector behind the
    // façade's back; B then takes the freed name as its rightful new
    // owner.
    await s.reset();
    const b = defineJsBlocks({ echo: () => 'B' }, { name: 'jsb-handover' });
    await b.attach(s.command);

    // A's re-attach must reject: an early-return "success" (what a
    // stale I-registered-this boolean produced) would leave A's blocks
    // silently answered by B's functions.
    await expect(a.attach(s.command)).rejects.toThrow(
      /already registered under 'jsb-handover'/,
    );

    // And A's detach must be a no-op — the live registration is B's,
    // whatever A once owned before the reset.
    await a.detach(s.command);
    expect(await s.command.listConnectors()).toContain('jsb-handover');
    expect(connectorRegistered('jsb-handover')).toBe(true);

    // B is untouched and still answers.
    const id = await b.addBlock(s.command, 'echo');
    await s.command.writeBlockInput(id, 'in', 1);
    expect(await until(() => lastOut(notes, id), "B's answer")).toBe('B');
  });

  it('serializes overlapping attach calls into one addConnector round-trip', async () => {
    const { session: s } = session();
    const jsBlocks = defineJsBlocks(
      { echo: (value) => value },
      { name: 'jsb-race' },
    );

    // Count the engine round-trips through a delegating wrapper — only
    // the two methods attach uses need to exist.
    let addCalls = 0;
    const counting = {
      listConnectors: () => s.command.listConnectors(),
      addConnector: (name: string) => {
        addCalls += 1;
        return s.command.addConnector(name);
      },
    } as unknown as EngineCommand;

    // Two same-tick attaches: unserialized, both would pass the
    // listConnectors check and the loser would surface the engine's
    // "already managed" error.
    await Promise.all([jsBlocks.attach(counting), jsBlocks.attach(counting)]);
    expect(addCalls).toBe(1);
    expect(await s.command.listConnectors()).toContain('jsb-race');
  });

  it('turns nulls into undefined across the bridge — a measured limit', async () => {
    const { session: s } = session();
    let received: unknown;
    const jsBlocks = defineJsBlocks(
      {
        probe: (value) => {
          received = value;
          return 'ok';
        },
      },
      { name: 'jsb-null' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'probe');
    await s.command.writeBlockInput(id, 'in', { x: null, z: [null] });

    await until(() => received, 'the probed value');
    // Keys and array slots survive, but the nulls arrive as undefined.
    // Pinned here so a bridge change that starts round-tripping nulls
    // shows up as a failing expectation.
    expect(Object.keys(received as object)).toEqual(['x', 'z']);
    expect(received).toEqual({ x: undefined, z: [undefined] });
  });

  it('coerces safe BigInts to numbers and faults on unsafe ones', async () => {
    const { session: s, notes } = session();
    let received: unknown;
    const jsBlocks = defineJsBlocks(
      {
        probe: (value) => {
          received = value;
          return value;
        },
      },
      { name: 'jsb-bigint' },
    );
    await jsBlocks.attach(s.command);

    // In the safe integer range a BigInt silently becomes a Number.
    const id = await jsBlocks.addBlock(s.command, 'probe');
    await s.command.writeBlockInput(id, 'in', 42n);
    const out = await until(() => lastOut(notes, id), 'the coerced value');
    expect(received).toBe(42);
    expect(out).toBe(42);

    // Outside it the bridge faults the block before the function runs.
    const id2 = await jsBlocks.addBlock(s.command, 'probe');
    await s.command.writeBlockInput(id2, 'in', 9007199254740993n);
    const fault = await until(() => lastFault(notes, id2), 'a bigint fault');
    expect(fault.faultReason).toContain(
      "can't be represented as a JavaScript number",
    );
  });

  it('round-trips arrays of dicts', async () => {
    const { session: s, notes } = session();
    let received: unknown;
    const jsBlocks = defineJsBlocks(
      {
        swap: (value) => {
          received = value;
          return (value as object[]).slice().reverse();
        },
      },
      { name: 'jsb-list' },
    );
    await jsBlocks.attach(s.command);

    const id = await jsBlocks.addBlock(s.command, 'swap');
    await s.command.writeBlockInput(id, 'in', [{ a: 1 }, { b: 2 }]);

    const out = await until(() => lastOut(notes, id), 'the list result');
    // In: dict elements became plain objects. Out: they are Maps again.
    expect(received).toEqual([{ a: 1 }, { b: 2 }]);
    const list = out as Map<string, unknown>[];
    expect(Array.isArray(list)).toBe(true);
    expect(list[0]).toBeInstanceOf(Map);
    expect(list[0].get('b')).toBe(2);
    expect(list[1].get('a')).toBe(1);
  });

  it('serves an ExternalOut sink through publish', async () => {
    const { session: s } = session();
    const seen: unknown[] = [];
    const jsBlocks = defineJsBlocks(
      {
        log: (value) => {
          seen.push(value);
        },
      },
      { name: 'jsb-sink' },
    );
    await jsBlocks.attach(s.command);

    // The sink direction needs no extra API: an ExternalOut bound to
    // the connector name and a function name publishes into it.
    const id = await s.command.addBlock('ExternalOut');
    await s.command.writeBlockInput(id, 'connector', 'jsb-sink');
    await s.command.writeBlockInput(id, 'address', 'log');
    await s.command.writeBlockInput(id, 'in', 7);

    await until(() => (seen.length > 0 ? true : undefined), 'the sink call');
    expect(seen).toContain(7);
  });

  it('faults an ExternalIn bound to the request-only connector', async () => {
    const { session: s, notes } = session();
    const jsBlocks = defineJsBlocks(
      { echo: (value) => value },
      { name: 'jsb-stream' },
    );
    await jsBlocks.attach(s.command);

    // There is nothing to subscribe to — the connector's subscribe
    // throws, and the ExternalIn block surfaces that as its fault
    // rather than sitting silent.
    const id = await s.command.addBlock('ExternalIn');
    await s.command.writeBlockInput(id, 'connector', 'jsb-stream');
    await s.command.writeBlockInput(id, 'address', 'echo');

    const fault = await until(() => lastFault(notes, id), 'a subscribe fault');
    expect(fault.faultReason).toContain('no value streams');
  });
});
