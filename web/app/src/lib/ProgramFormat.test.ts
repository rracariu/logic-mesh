// The upgrade of saved programs to the current format. Every bundled
// example is in the current format, so these tests are what exercises
// the path that saves from logic-mesh 1.x and earlier editor builds
// take on load.
import { PROGRAM_FORMAT_VERSION } from 'logic-mesh';
import { describe, expect, it } from 'vitest';
import program10 from '../../../../src/base/program/testdata/program-1.0.json';
import { upgradeFormat } from './ProgramFormat';

const FIRST = '00000000-0000-4000-8000-000000000001';
const SECOND = '00000000-0000-4000-8000-000000000002';

const slider = { kind: 'Slider', config: { min: 0, max: 10, step: 0.5 } };

/** A program as an editor build from before format versions saved it. */
function legacySave() {
  return {
    name: 'legacy',
    blocks: {
      [FIRST]: {
        name: 'ExternalIn',
        lib: 'core',
        positions: { x: 81, y: -72.5 },
        widget: slider,
        inputs: {
          connector: { value: 'ui', isConnected: false },
          address: { value: FIRST, isConnected: false },
        },
        outputs: { out: { value: 3 } },
      },
      [SECOND]: {
        name: 'Add',
        lib: 'core',
        label: 'sum',
        positions: { x: 267, y: 25 },
        inputs: { in0: { isConnected: true } },
      },
    },
  };
}

describe('upgradeFormat', () => {
  it('brings a save from before format versions to the current format', () => {
    const saved = legacySave();

    const program = upgradeFormat(saved);

    // In place: the object prepare() built nodes from is the one
    // pushToEngine loads.
    expect(program).toBe(saved);
    expect(program).toStrictEqual({
      version: PROGRAM_FORMAT_VERSION,
      name: 'legacy',
      blocks: {
        [FIRST]: {
          name: 'ExternalIn',
          lib: 'core',
          position: { x: 81, y: -72.5 },
          editor: { widget: slider },
          inputs: {
            connector: { value: 'ui', isConnected: false },
            address: { value: FIRST, isConnected: false },
          },
          outputs: { out: { value: 3, isConnected: false } },
        },
        [SECOND]: {
          name: 'Add',
          lib: 'core',
          label: 'sum',
          position: { x: 267, y: 25 },
          inputs: { in0: { value: null, isConnected: true } },
        },
      },
      links: {},
    });
  });

  it('upgrades the logic-mesh 1.0 save the engine tests load', () => {
    const saved = structuredClone(program10);

    const program = upgradeFormat(saved);

    expect(program.version).toBe(PROGRAM_FORMAT_VERSION);
    expect(program.blocks[FIRST].position).toStrictEqual({ x: 81, y: -72.5 });
    expect(program.blocks[FIRST]).not.toHaveProperty('positions');
    expect(program.blocks[FIRST].outputs?.out).toStrictEqual({
      value: 19,
      isConnected: false,
    });
    expect(program.links).toStrictEqual(program10.links);
  });

  it('keeps what a legacy block already has in the current shape', () => {
    const saved = legacySave();
    const first = saved.blocks[FIRST] as Record<string, unknown>;
    first.position = { x: 1, y: 2 };
    first.editor = { note: 'kept' };

    const program = upgradeFormat(saved);

    const block = program.blocks[FIRST];
    expect(block.position).toStrictEqual({ x: 1, y: 2 });
    expect(block).not.toHaveProperty('positions');
    expect(block.editor).toStrictEqual({ note: 'kept', widget: slider });
    expect(block).not.toHaveProperty('widget');
  });

  it('reads version 0 as a save from before format versions', () => {
    const saved = { ...legacySave(), version: 0 };

    const program = upgradeFormat(saved);

    expect(program.version).toBe(PROGRAM_FORMAT_VERSION);
    expect(program.blocks[FIRST].editor).toStrictEqual({ widget: slider });
  });

  it('fills in missing blocks and links', () => {
    expect(upgradeFormat({ name: 'empty' })).toStrictEqual({
      version: PROGRAM_FORMAT_VERSION,
      name: 'empty',
      blocks: {},
      links: {},
    });
  });

  it('returns a program in the current format as it is', () => {
    const saved = {
      version: PROGRAM_FORMAT_VERSION,
      blocks: {
        [FIRST]: {
          name: 'ExternalIn',
          lib: 'core',
          position: { x: 0, y: 0 },
          editor: { widget: slider },
        },
      },
      links: {},
    };
    const copy = structuredClone(saved);

    expect(upgradeFormat(saved)).toBe(saved);
    expect(saved).toStrictEqual(copy);
  });

  it('rejects a program from a newer format', () => {
    const newer = PROGRAM_FORMAT_VERSION + 1;
    const saved = { version: newer, blocks: {}, links: {} };

    expect(() => upgradeFormat(saved)).toThrow(
      `Program format version ${newer} is newer than this editor reads`,
    );
  });
});
