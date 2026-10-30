// The editor's program save format, and the upgrade of older saves to
// it. Kept apart from Program.ts, which starts the engine on import, so
// it can be tested on its own.
import {
  PROGRAM_FORMAT_VERSION,
  type PinValue,
  type Position,
  type Program,
  type ProgramBlock,
} from 'logic-mesh';
import type { Widget } from './Block';

/** The editor metadata this editor saves with a block. */
export type BlockEditor = {
  /** Widget identity, for ExternalIn/ExternalOut blocks placed as widgets. */
  widget?: Widget;
};

/** A program as this editor saves and loads it. */
export type EditorProgram = Program<BlockEditor>;

/** A block as this editor saves and loads it. */
export type EditorBlock = ProgramBlock<BlockEditor>;

/**
 * The version a save from before format versions reads as, as the
 * engine reads it.
 */
const LEGACY_FORMAT_VERSION = 0;

/**
 * A block as a save from before format versions wrote it: the position
 * keyed `positions`, the widget beside the block's fields rather than in
 * its editor metadata, and pins whose value or connected flag may be
 * missing.
 */
export type LegacyBlock = Omit<EditorBlock, 'inputs' | 'outputs'> & {
  positions?: Position;
  widget?: Widget;
  inputs?: Record<string, Partial<PinValue>>;
  outputs?: Record<string, Partial<PinValue>>;
};

/** A save from before format versions, see {@link LegacyBlock}. */
export type LegacyProgram = Omit<
  EditorProgram,
  'version' | 'blocks' | 'links'
> & {
  version?: typeof LEGACY_FORMAT_VERSION;
  blocks?: Record<string, LegacyBlock>;
  links?: EditorProgram['links'];
};

/**
 * Bring a saved program to the current format, in place, so the same
 * object is valid for both node building and `pushToEngine`.
 *
 * A save from before format versions — one without `version` (or with
 * version 0, as the engine reads it), written by logic-mesh 1.x or an
 * earlier build of this editor — moves its `positions` to `position`
 * and a block's `widget` into its editor metadata, and gets the pin
 * fields it left out. A program in the current format is returned as
 * it is. A program from a newer format is rejected before anything is
 * built from it.
 */
export function upgradeFormat(saved: unknown): EditorProgram {
  const program = saved as EditorProgram | LegacyProgram;
  const version = program.version ?? LEGACY_FORMAT_VERSION;
  if (version > PROGRAM_FORMAT_VERSION) {
    throw new Error(
      `Program format version ${version} is newer than this editor reads (${PROGRAM_FORMAT_VERSION})`,
    );
  }
  if (version !== LEGACY_FORMAT_VERSION) {
    return program as EditorProgram;
  }

  const legacy = program as LegacyProgram;
  const blocks = (legacy.blocks ??= {});
  for (const block of Object.values(blocks)) {
    if (block.positions) {
      block.position ??= block.positions;
      delete block.positions;
    }
    if (block.widget) {
      block.editor = { ...block.editor, widget: block.widget };
      delete block.widget;
    }
    for (const pins of [block.inputs, block.outputs]) {
      for (const pin of Object.values(pins ?? {})) {
        pin.value ??= null;
        pin.isConnected ??= false;
      }
    }
  }
  legacy.links ??= {};
  return Object.assign(legacy, {
    version: PROGRAM_FORMAT_VERSION,
  }) as EditorProgram;
}
