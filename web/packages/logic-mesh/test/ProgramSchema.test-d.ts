// Type-level tests of the program schema, checked by `tsc` through
// vitest's typecheck mode (see vitest.config.ts) and never run. A parsed
// program must be exactly the package's `Program` type, so the schema
// and the hand-written types cannot drift apart unnoticed.
import { describe, expectTypeOf, it } from 'vitest';
import { z } from 'zod';
import type { Program } from '../src/index';
import { ProgramSchema, programSchema } from '../src/index';

describe('program schema types', () => {
  it('parse to a Program', () => {
    expectTypeOf<z.output<typeof ProgramSchema>>().toEqualTypeOf<Program>();
    expectTypeOf(ProgramSchema.parse).returns.toEqualTypeOf<Program>();
  });

  it('parse to a Program of the editor schema given', () => {
    const EditorProgram = programSchema(
      z.object({ widget: z.object({ kind: z.string() }).optional() }),
    );
    expectTypeOf(EditorProgram.parse).returns.toEqualTypeOf<
      Program<{ widget?: { kind: string } }>
    >();
  });

  it('take an editor schema of a JSON object only', () => {
    // @ts-expect-error -- editor metadata is a JSON object, not an array.
    programSchema(z.array(z.string()));
    // @ts-expect-error -- nor a string.
    programSchema(z.string());
  });
});
