import type {
  EditorMetadata,
  LinkData,
  Program,
  ProgramBlock,
} from './index.js';
import { PROGRAM_FORMAT_VERSION } from './ProgramVersion.js';
import { z, type ZodType } from 'zod';

/** The version a save from before format versions reads as. */
const LEGACY_FORMAT_VERSION = 0;

// The schemas below mirror how the engine reads a program (the serde
// shape of `base::program` in the Rust crate): the same keys are
// required, `null` reads as absent where the engine reads an `Option`,
// and keys the format does not have are dropped. Each transform's
// result is annotated with the package's own type, so a parsed program
// is exactly a `Program`.

const PositionSchema = z.object({ x: z.number(), y: z.number() });

const PinValueSchema = z.object({
  // A Haystack value in its JSON encoding. The engine also checks that
  // it decodes as one, which no JSON shape can tell.
  value: z.json(),
  isConnected: z.boolean().default(false),
});

const LinkDataSchema = z
  .object({
    id: z.string().nullish(),
    sourceBlockPinName: z.string(),
    sourceBlockUuid: z.string(),
    targetBlockPinName: z.string(),
    targetBlockUuid: z.string(),
  })
  .transform(({ id, ...link }): LinkData =>
    id == null ? link : { id, ...link },
  );

const EditorMetadataSchema: ZodType<EditorMetadata> = z.record(
  z.string(),
  z.json(),
);

function programBlockSchema<Editor extends EditorMetadata>(
  editor: ZodType<Editor>,
) {
  return (
    z
      .object({
        name: z.string(),
        lib: z.string(),
        ver: z.string().nullish(),
        label: z.string().nullish(),
        position: PositionSchema.nullish(),
        // The key a save from before format versions used.
        positions: PositionSchema.nullish(),
        editor: editor.nullish(),
        inputs: z.record(z.string(), PinValueSchema).optional(),
        outputs: z.record(z.string(), PinValueSchema).optional(),
      })
      // As in the engine, which reads `positions` as an alias, naming
      // both keys is an error, even when one of them is `null`.
      .superRefine((block, ctx) => {
        if ('position' in block && 'positions' in block) {
          ctx.addIssue({
            code: 'custom',
            message: 'duplicate field `position`',
            path: ['positions'],
          });
        }
      })
      .transform((raw): ProgramBlock<Editor> => {
        const block: ProgramBlock<Editor> = { name: raw.name, lib: raw.lib };
        const position = raw.position ?? raw.positions;
        if (raw.ver != null) block.ver = raw.ver;
        if (raw.label != null) block.label = raw.label;
        if (position != null) block.position = position;
        if (raw.editor != null) block.editor = raw.editor;
        if (raw.inputs) block.inputs = raw.inputs;
        if (raw.outputs) block.outputs = raw.outputs;
        return block;
      })
  );
}

/**
 * A Zod schema of a saved program: what `getProgram` returns and
 * `loadProgram` takes, checked without the engine. A tool can validate
 * a program file, or a server the programs it stores, before any wasm
 * is loaded, and each problem comes with its path in the program.
 *
 * It reads a program as the engine does:
 *
 * - a program without `version`, a save from logic-mesh 1.x, parses as
 *   version 0, and a block's `positions` as its `position`;
 * - a missing `blocks`, `links` or pin `isConnected` reads as empty or
 *   `false`, and a `null` optional field as absent;
 * - keys the format does not have are dropped;
 * - a program from a newer format fails on `version`, with the message
 *   `loadProgram` rejects it with. That is its first issue; the rest of
 *   its shape is checked too.
 *
 * A program that parses can still fail to load: the shape says nothing
 * of whether its block types are registered, its pins exist, its ids
 * are UUIDs or its pin values decode as Haystack values. Those are the
 * engine's checks.
 *
 * Without an argument, editor metadata is any JSON object, as the
 * engine takes it. Pass a schema for your own `Editor` type to check
 * that too, and to type the result as `Program<Editor>`. The engine
 * keeps editor metadata as it is, so to keep keys your schema does not
 * name, build it with `z.looseObject`.
 *
 * Pin values and editor metadata are read as JSON, what `JSON.parse`
 * returns: an `undefined` or a function in them fails to parse.
 *
 * For tools outside JavaScript, `z.toJSONSchema(ProgramSchema, { io:
 * 'input' })` gives the format as a JSON Schema, without the checks
 * JSON Schema cannot express (a duplicate `position`, and the message
 * of a newer `version`).
 *
 * ```ts
 * import { ProgramSchema, programSchema } from 'logic-mesh';
 * import { z } from 'zod';
 *
 * const result = ProgramSchema.safeParse(JSON.parse(text));
 * if (!result.success) console.error(z.prettifyError(result.error));
 *
 * const EditorProgram = programSchema(
 *   z.looseObject({ widget: z.object({ kind: z.string() }).optional() }),
 * );
 * const program = EditorProgram.parse(saved); // Program<{ widget?: … }>
 * ```
 */
export function programSchema(): ZodType<Program>;
export function programSchema<Editor extends EditorMetadata>(
  editor: ZodType<Editor>,
): ZodType<Program<Editor>>;
export function programSchema(
  editor: ZodType<EditorMetadata> = EditorMetadataSchema,
): ZodType<Program> {
  return z
    .object({
      // First, so that a newer format's version is its first issue.
      version: z
        .int()
        .min(0)
        .max(PROGRAM_FORMAT_VERSION, {
          error: (issue) =>
            `Program format version ${issue.input} is not supported; the newest supported is ${PROGRAM_FORMAT_VERSION}`,
        })
        .optional(),
      name: z.string().nullish(),
      description: z.string().nullish(),
      blocks: z.record(z.string(), programBlockSchema(editor)).optional(),
      links: z.record(z.string(), LinkDataSchema).optional(),
    })
    .transform((raw): Program => {
      const program: Program = {
        version: raw.version ?? LEGACY_FORMAT_VERSION,
        blocks: raw.blocks ?? {},
        links: raw.links ?? {},
      };
      if (raw.name != null) program.name = raw.name;
      if (raw.description != null) program.description = raw.description;
      return program;
    });
}

/**
 * {@link programSchema} with any JSON object as editor metadata.
 */
export const ProgramSchema: ZodType<Program> = programSchema();
