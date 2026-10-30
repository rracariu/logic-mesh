// A module of its own, so that importing the version builds no schema
// and pulls in no zod: ProgramSchema.ts, which needs it at load time,
// imports it from here rather than from the entry it is re-exported by.

/**
 * The program save format version this package writes, and the newest
 * its engine loads. `getProgram` returns programs in this version;
 * `loadProgram` rejects a program with a higher one.
 *
 * A program with no `version` is a save from before format versions
 * (logic-mesh 1.x): the same shape, except that a block's position is
 * keyed `positions`. `loadProgram` still loads it.
 */
export const PROGRAM_FORMAT_VERSION = 1;
