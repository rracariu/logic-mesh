// Program save and load across the wasm boundary, against the real
// engine in Node: what `loadProgram` takes, `getProgram` returns as
// plain JSON in the current format. Each test runs its own engine
// session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import type { EngineSession, Program } from '../src/index';
import { PROGRAM_FORMAT_VERSION, startEngine } from '../src/index';

const sessions: EngineSession[] = [];

afterEach(async () => {
  await Promise.all(sessions.splice(0).map((s) => s.stop()));
});

function session(): EngineSession {
  const s = startEngine({ sleepDuration: 10 });
  sessions.push(s);
  return s;
}

const FIRST = '00000000-0000-4000-8000-000000000001';
const SECOND = '00000000-0000-4000-8000-000000000002';
const LINK = '00000000-0000-4000-8000-0000000000aa';

/** Editor metadata using every JSON kind, nested. */
const editor = {
  widget: {
    kind: 'MultiChart',
    config: {
      series: [{ label: 'OAT' }, { label: '', address: SECOND }],
      min: -40,
      max: Number.MAX_SAFE_INTEGER,
      step: 0.1,
      ratio: 1e-7,
      visible: true,
      address: null,
      empty: {},
      none: [],
    },
    configSources: { min: FIRST },
  },
  // Haystack-looking keys stay plain JSON.
  marker: { _kind: 'marker' },
  nested: [[1, [2.5, null]], { a: { b: { c: 'd' } } }],
};

function sample(): Program {
  return {
    version: PROGRAM_FORMAT_VERSION,
    name: 'round trip',
    description: 'Loaded and saved.',
    blocks: {
      [FIRST]: {
        name: 'Add',
        lib: 'core',
        label: 'first',
        position: { x: 10, y: -20.5 },
        editor,
        inputs: {
          in0: { value: 2, isConnected: false },
          in1: { value: 3, isConnected: false },
        },
      },
      [SECOND]: { name: 'Add', lib: 'core', position: { x: 200, y: 0 } },
    },
    links: {
      [LINK]: {
        id: LINK,
        sourceBlockUuid: FIRST,
        sourceBlockPinName: 'out',
        targetBlockUuid: SECOND,
        targetBlockPinName: 'in0',
      },
    },
  };
}

describe('program save and load', () => {
  it('returns editor metadata exactly as it was loaded', async () => {
    const { command } = session();
    await command.loadProgram(sample());

    const saved = (await command.getProgram()) as Program;
    expect(saved.blocks[FIRST].editor).toStrictEqual(editor);
    expect(saved.blocks[SECOND].editor).toBeUndefined();
  });

  it('returns plain JSON in the current format', async () => {
    const { command } = session();
    await command.loadProgram(sample());

    const saved = (await command.getProgram()) as Program;
    // Plain objects and nulls, not Maps and undefined.
    expect(saved).toStrictEqual(JSON.parse(JSON.stringify(saved)));
    expect(saved.version).toBe(PROGRAM_FORMAT_VERSION);
    expect(saved.name).toBe('round trip');
    const first = saved.blocks[FIRST];
    expect(first.ver).toBe('1.0.0');
    expect(first.label).toBe('first');
    expect(first.position).toStrictEqual({ x: 10, y: -20.5 });
    expect(first.inputs?.in0).toStrictEqual({ value: 2, isConnected: false });
    expect(saved.links[LINK]).toStrictEqual(sample().links[LINK]);

    // What getProgram returns loads back and saves the same.
    await command.resetEngine();
    await command.loadProgram(saved);
    const again = (await command.getProgram()) as Program;
    expect(again.blocks[FIRST].editor).toStrictEqual(editor);
    expect(again.links).toStrictEqual(saved.links);
  });

  it('loads a program saved before format versions', async () => {
    const { command } = session();
    // The logic-mesh 1.0 shape: no version, `positions`, outputs
    // without `isConnected`, links keyed by id without naming it.
    await command.loadProgram({
      name: 'legacy',
      blocks: {
        [FIRST]: {
          name: 'Add',
          lib: 'core',
          positions: { x: 81, y: -72 },
          inputs: { in0: { value: 12, isConnected: false } },
          outputs: { out: { value: 12 } },
        },
      },
      links: {},
    });

    const saved = (await command.getProgram()) as Program;
    expect(saved.version).toBe(PROGRAM_FORMAT_VERSION);
    expect(saved.blocks[FIRST].position).toStrictEqual({ x: 81, y: -72 });
    expect(saved.blocks[FIRST]).not.toHaveProperty('positions');
  });

  it('rejects a program from a newer format before reading it', async () => {
    const { command } = session();
    const newer = PROGRAM_FORMAT_VERSION + 1;

    await expect(
      command.loadProgram({
        version: newer,
        blocks: { [FIRST]: { type: 'a shape this engine does not know' } },
      }),
    ).rejects.toMatch(`version ${newer} is not supported`);
    const saved = (await command.getProgram()) as Program;
    expect(saved.blocks).toStrictEqual({});
  });
});
