// The program schema against the real engine in Node: it must read a
// program as `loadProgram` does — accept what the engine loads, reject
// what it refuses — and leave what `getProgram` returns unchanged.
// Each engine test runs its own session, stopped afterwards.
import { afterEach, describe, expect, it } from 'vitest';
import { z } from 'zod';
import program10 from '../../../../src/base/program/testdata/program-1.0.json';
import type { EngineSession, Program } from '../src/index';
import {
  PROGRAM_FORMAT_VERSION,
  ProgramSchema,
  programSchema,
  startEngine,
} from '../src/index';

const sessions: EngineSession[] = [];

afterEach(async () => {
  await Promise.all(sessions.splice(0).map((s) => s.stop()));
});

function session(): EngineSession {
  const s = startEngine({ scanInterval: 10 });
  sessions.push(s);
  return s;
}

const FIRST = '00000000-0000-4000-8000-000000000001';
const SECOND = '00000000-0000-4000-8000-000000000002';
const LINK = '00000000-0000-4000-8000-0000000000aa';

/** A block and a link the engine loads, with `extra` merged in. */
function program(extra: {
  program?: object;
  block?: object;
  pin?: object;
  link?: object;
}): Record<string, unknown> {
  return {
    version: PROGRAM_FORMAT_VERSION,
    blocks: {
      [FIRST]: {
        name: 'Add',
        lib: 'core',
        inputs: { in0: { value: 2, isConnected: false, ...extra.pin } },
        ...extra.block,
      },
      [SECOND]: { name: 'Add', lib: 'core' },
    },
    links: {
      [LINK]: {
        sourceBlockUuid: FIRST,
        sourceBlockPinName: 'out',
        targetBlockUuid: SECOND,
        targetBlockPinName: 'in0',
        ...extra.link,
      },
    },
    ...extra.program,
  };
}

describe('ProgramSchema', () => {
  it('leaves a program the engine saved unchanged', async () => {
    const { command } = session();
    await command.loadProgram({
      ...program({
        program: { name: 'demo', description: 'Saved by the engine.' },
        block: {
          label: 'sum',
          position: { x: 10, y: -20.5 },
          editor: {
            widget: { kind: 'Slider', config: { min: 0, step: 0.5 } },
            nested: [[1, null], { a: 'b' }],
          },
        },
        link: { id: LINK },
      }),
    } as Program);

    const saved = (await command.getProgram()) as Program;
    expect(ProgramSchema.parse(saved)).toStrictEqual(saved);
  });

  it('reads a 1.0 save as the engine does', () => {
    const parsed = ProgramSchema.parse(program10);
    expect(parsed.version).toBe(0);
    const [block] = Object.values(parsed.blocks);
    expect(block.position).toBeDefined();
    expect(block).not.toHaveProperty('positions');
    for (const pin of Object.values(block.outputs ?? {})) {
      expect(typeof pin.isConnected).toBe('boolean');
    }
  });

  it('reads missing collections as empty and null fields as absent', () => {
    expect(ProgramSchema.parse({})).toStrictEqual({
      version: 0,
      blocks: {},
      links: {},
    });

    const parsed = ProgramSchema.parse(
      program({
        program: { name: null, description: null },
        block: { ver: null, label: null, position: null, editor: null },
        pin: { isConnected: undefined },
        link: { id: null },
      }),
    );
    expect(parsed).not.toHaveProperty('name');
    expect(parsed).not.toHaveProperty('description');
    expect(parsed.blocks[FIRST]).toStrictEqual({
      name: 'Add',
      lib: 'core',
      inputs: { in0: { value: 2, isConnected: false } },
    });
    expect(parsed.links[LINK]).not.toHaveProperty('id');
  });

  it('drops keys the format does not have, as a load does', async () => {
    const input = program({
      program: { extra: 1 },
      block: { extra: 2 },
      pin: { extra: 3 },
      link: { extra: 4 },
    });
    const parsed = ProgramSchema.parse(input);
    expect(JSON.stringify(parsed)).not.toContain('extra');

    const { command } = session();
    await command.loadProgram(input as Program);
    expect(JSON.stringify(await command.getProgram())).not.toContain('extra');
  });

  it('fails a newer format on its version, with the message of a load', async () => {
    const newer = {
      version: PROGRAM_FORMAT_VERSION + 1,
      blocks: { [FIRST]: { type: 'a shape this engine does not know' } },
    };
    const result = ProgramSchema.safeParse(newer);
    expect(result.success).toBe(false);
    const [first] = result.error!.issues;
    expect(first.path).toStrictEqual(['version']);

    const { command } = session();
    await expect(command.loadProgram(newer as Program)).rejects.toMatchObject({
      code: 'UnsupportedProgramVersion',
      message: first.message,
    });
  });

  // Each case is a program the engine refuses to read. The schema must
  // refuse it too, so a program that passes the schema is not turned
  // away by a load for its shape.
  it.each([
    ['a negative version', { program: { version: -1 } }],
    ['a version that is not a number', { program: { version: '1' } }],
    ['blocks that are not a map', { program: { blocks: 42 } }],
    ['null blocks', { program: { blocks: null } }],
    ['a block without its library', { block: { lib: undefined } }],
    [
      'a block naming position and positions',
      {
        block: { position: null, positions: { x: 1, y: 2 } },
      },
    ],
    [
      'a position that is not numbers',
      { block: { position: { x: '1', y: 2 } } },
    ],
    ['editor metadata that is not an object', { block: { editor: 'str' } }],
    ['null inputs', { block: { inputs: null } }],
    ['a pin without a value', { pin: { value: undefined } }],
    [
      'a pin whose isConnected is not a boolean',
      { pin: { isConnected: 'yes' } },
    ],
    ['a link without its target', { link: { targetBlockUuid: undefined } }],
  ])('rejects %s, as a load does', async (_, extra) => {
    const input = JSON.parse(JSON.stringify(program(extra)));
    expect(ProgramSchema.safeParse(input).success).toBe(false);

    const { command } = session();
    await expect(command.loadProgram(input)).rejects.toMatchObject({
      code: 'InvalidProgram',
    });
  });

  // The format's maps are JSON objects. Across the wasm boundary
  // `loadProgram` reads an array where it expects one as a list of map
  // entries (`[]` as `{}`, `['a']` as `{ a: null }`), which the format
  // does not allow and the engine's JSON reader rejects. The schema
  // follows the format.
  it('rejects an array where the format has an object', () => {
    for (const extra of [
      { program: { blocks: [] } },
      { block: { editor: [] } },
      { block: { inputs: ['in0'] } },
    ]) {
      expect(ProgramSchema.safeParse(program(extra)).success).toBe(false);
    }
  });

  it('gives a JSON Schema of the format', () => {
    const schema = z.toJSONSchema(ProgramSchema, { io: 'input' }) as {
      properties: Record<string, Record<string, unknown>>;
      required?: string[];
    };
    expect(schema.properties.version).toMatchObject({
      type: 'integer',
      minimum: 0,
      maximum: PROGRAM_FORMAT_VERSION,
    });
    expect(schema.required ?? []).toStrictEqual([]);
    const block = schema.properties.blocks.additionalProperties as {
      required: string[];
    };
    expect(block.required).toStrictEqual(['name', 'lib']);
  });
});

describe('programSchema', () => {
  const EditorProgram = programSchema(
    z.looseObject({
      widget: z.object({ kind: z.string() }).optional(),
    }),
  );

  it('checks editor metadata with the schema given', () => {
    const result = EditorProgram.safeParse(
      program({ block: { editor: { widget: { kind: 3 } } } }),
    );
    expect(result.success).toBe(false);
    expect(result.error!.issues[0].path).toStrictEqual([
      'blocks',
      FIRST,
      'editor',
      'widget',
      'kind',
    ]);
  });

  it('keeps the editor keys a loose schema does not name', () => {
    const parsed = EditorProgram.parse(
      program({ block: { editor: { widget: { kind: 'Led' }, zoom: 2 } } }),
    );
    expect(parsed.blocks[FIRST].editor).toStrictEqual({
      widget: { kind: 'Led' },
      zoom: 2,
    });
  });
});
