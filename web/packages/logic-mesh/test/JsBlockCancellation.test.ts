// Regression tests for the `registerBlock`-path JsBlock under mailbox
// cancellation, against the real wasm engine in Node.
//
// The block actor races `execute()` against the block's command mailbox
// with a biased select: ANY engine command addressed to the block
// (inspect, pin write, link wiring) drops the in-flight `execute()`
// future at its current await point. A JsBlock has a second await after
// the input wait — the JS function's Promise — so a command landing
// while the Promise is pending used to discard the drained reaction
// permanently: the inputs were already consumed from the watch
// channels, the Promise's eventual result went nowhere, and no new
// `execute` fired until fresh input arrived. The fix holds the drained
// reaction on the block and re-invokes the JS function on the next
// cycle (at-least-once), which these tests pin down.
//
// Determinism: the executor is gated, not delayed. Each invocation
// resolves a `started` deferred and then parks on a shared `gate`
// Promise the test releases only after the cancelling command's round
// trip has completed — the command reply is sent by the actor *after*
// the execute future was dropped, so awaiting the write proves the
// cancellation landed mid-Promise. No fixed-duration window remains.
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

/** The latest value a notification reported for the block's `out` pin. */
function lastOut(notes: BlockNotification[], id: string): unknown {
  return notes
    .filter((n) => n.id === id)
    .flatMap((n) => n.changes)
    .filter((c) => c.name === 'out')
    .at(-1)?.value;
}

/** A promise plus its externally-callable resolver. */
function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void;
  const promise = new Promise<void>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

/** The block-desc registry is process-wide, so every test registers
 * its block under a fresh name — same idiom as the connector tests. */
let blockSerial = 0;

/**
 * A session with a watch, plus a registered two-input gated block:
 * the first pin is kind Null (so the Null a link seed pushes drains
 * without a type conversion) and fed over a link; the second pin
 * exists for an engine command to hit, landing in the block's mailbox
 * mid-flight. Non-numeric first-pin cycles (the link seed) are
 * ignored. Returns handles to drive and observe the block.
 */
async function gatedBlockSession(pins: { drive: string; poke: string }) {
  const session = createEngineSession({ sleepDuration: 10 });
  sessions.push(session);

  const name = `Gated${blockSerial++}`;
  const desc: BlockDesc = {
    name,
    dis: 'Gated',
    lib: 'cancel-test',
    ver: '0.0.1',
    category: 'Test',
    doc: `Doubles \`${pins.drive}\` once the gate opens.`,
    implementation: 'external',
    inputs: [
      { name: pins.drive, kind: 'null' },
      { name: pins.poke, kind: 'number' },
    ],
    outputs: [{ name: 'out', kind: 'number' }],
  };

  // Every numeric invocation records BOTH pin values (unset arrives
  // nullish; normalized to null so the assertions are stable).
  const calls: unknown[][] = [];
  const gate = deferred();
  let started = deferred();
  session.engine.registerBlock(desc, () => async (inputs: unknown[]) => {
    const value = inputs[0];
    if (typeof value !== 'number') return;
    calls.push([value, inputs[1] ?? null]);
    started.resolve();
    await gate.promise;
    return [value * 2];
  });

  session.start();
  const notes: BlockNotification[] = [];
  session.watch((n) => notes.push(n));

  const jsId = await session.command.addBlock(name, undefined, 'cancel-test');

  // A reactive (non-`always`) block only fires on watch traffic, and
  // an engine pin write is a direct cache write that produces none —
  // so the trigger must travel over a link. The Add block never
  // executes here (its inputs stay unconnected); it is just an output
  // pin whose writes propagate through the link.
  const srcId = await session.command.addBlock('Add');
  await session.command.createLink(srcId, jsId, 'out', pins.drive);

  return {
    session,
    notes,
    calls,
    jsId,
    gate,
    /** Awaits the start of the next invocation after this call. */
    nextStart: () => {
      started = deferred();
      return started.promise;
    },
    /** Fires the block by pushing a value through the link. */
    drive: (value: number) =>
      session.command.writeBlockOutput(srcId, 'out', value),
  };
}

describe('registered JS block under mailbox cancellation', () => {
  it('re-invokes a call cancelled mid-Promise, then goes quiescent', async () => {
    const s = await gatedBlockSession({ drive: 'in', poke: 'cfg' });

    const firstStart = s.nextStart();
    await s.drive(21);
    await firstStart; // invocation 1 is parked on the gate

    // Land an engine command in the block's mailbox. The biased select
    // in the block actor prefers the mailbox, dropping the in-flight
    // `execute` at the Promise await; the command's reply is sent
    // after the drop, so this await proves the cancellation happened.
    const retryStart = s.nextStart();
    await s.session.command.writeBlockInput(s.jsId, 'cfg', 1);

    // The retry re-invokes the function without fresh input...
    await retryStart;
    s.gate.resolve();

    // ...and its result reaches the output. Pre-fix this timed out —
    // the drained reaction was gone and the block parked waiting for
    // input that never came. (Invocation 1's Promise also resolves on
    // the gate, but its JsFuture was dropped: only the completed
    // retry commits outputs.)
    const out = await until(
      () => lastOut(s.notes, s.jsId),
      'the doubled value',
    );
    expect(out).toBe(42);

    // Quiescence: the one cancellation costs exactly one duplicate —
    // two invocations total, and the count stays there over a generous
    // idle window (polled; a retry loop would re-fire within the
    // engine's 10 ms cadence many times over).
    const idleUntil = Date.now() + 750;
    while (Date.now() < idleUntil) {
      expect(s.calls.length).toBe(2);
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
    expect(s.calls.map(([a]) => a)).toEqual([21, 21]);
  });

  it('a value written by the cancelling command supersedes the held reaction', async () => {
    const s = await gatedBlockSession({ drive: 'a', poke: 'b' });

    const firstStart = s.nextStart();
    await s.drive(21);
    await firstStart; // invocation 1 saw b unset

    // The cancelling command is itself a pin write on the block: a
    // direct cache write with no watch traffic. The retry must read
    // the freshest caches — the new `b` rides along instead of the
    // reaction replaying its stale drain (ExternalOut's "a fresh
    // value replaces any held retry", extended to cache writes).
    const retryStart = s.nextStart();
    await s.session.command.writeBlockInput(s.jsId, 'b', 5);
    await retryStart;
    s.gate.resolve();

    const out = await until(
      () => lastOut(s.notes, s.jsId),
      'the doubled value',
    );
    expect(out).toBe(42);
    expect(s.calls).toEqual([
      [21, null],
      [21, 5],
    ]);
  });
});

describe('registerBlock pin-kind validation', () => {
  it('rejects an unrecognized pin kind instead of degrading it to Null', () => {
    const session = createEngineSession({ sleepDuration: 10 });
    sessions.push(session);

    const desc = {
      name: `Strict${blockSerial++}`,
      dis: 'Strict',
      lib: 'cancel-test',
      ver: '0.0.1',
      category: 'Test',
      doc: 'Never registers: the pin kind is not a haystack kind name.',
      implementation: 'external',
      inputs: [{ name: 'in1', kind: 'Number' }], // must be lowercase 'number'
      outputs: [],
    };

    // Deliberately bypasses the TS `Kind` union — the point is that the
    // ENGINE also rejects what the type system rejects, instead of
    // silently registering an accepts-anything Null pin.
    expect(() =>
      session.engine.registerBlock(desc as unknown as BlockDesc, undefined),
    ).toThrowError(/pin 'in1'.*Invalid kind: Number/);
  });
});
