// Type-level tests of the program types, checked by `tsc` through
// vitest's typecheck mode (see vitest.config.ts) and never run.
import { describe, expectTypeOf, it } from 'vitest';
import type { EditorMetadata, Program, ProgramBlock } from '../src/index';

type Widget = { kind: string; config?: Record<string, unknown> };
type BlockEditor = { widget?: Widget };

describe('editor metadata types', () => {
  it('take a JSON object type', () => {
    const block: ProgramBlock<BlockEditor> = {
      name: 'ExternalIn',
      lib: 'core',
      editor: { widget: { kind: 'Slider', config: { step: 0.5 } } },
    };
    expectTypeOf(block.editor).toEqualTypeOf<BlockEditor | undefined>();
    expectTypeOf<Program<BlockEditor>['blocks'][string]>().toEqualTypeOf<
      ProgramBlock<BlockEditor>
    >();
    expectTypeOf<ProgramBlock['editor']>().toEqualTypeOf<
      EditorMetadata | undefined
    >();
  });

  // Regression: the bound was `object`, which arrays, functions and
  // class instances satisfy, although the engine reads only a JSON
  // object: it rejects an array, and a `Date` comes back as `{}`.
  it('reject what is not a plain JSON object', () => {
    // @ts-expect-error -- an array is not a JSON object.
    expectTypeOf<ProgramBlock<string[]>>().not.toBeNever();
    // @ts-expect-error -- a class instance is not a plain object.
    expectTypeOf<ProgramBlock<Date>>().not.toBeNever();
    // @ts-expect-error -- a function is not a JSON object.
    expectTypeOf<Program<() => void>>().not.toBeNever();

    const block: ProgramBlock = {
      name: 'Add',
      lib: 'core',
      // @ts-expect-error -- the default metadata type is an object too.
      editor: ['widget'],
    };
    expectTypeOf(block).toEqualTypeOf<ProgramBlock>();
  });
});
