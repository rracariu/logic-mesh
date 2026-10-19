import type { Edge, Node } from '@xyflow/svelte';
import {
  PROGRAM_FORMAT_VERSION,
  type PinValue,
  type Position,
  type Program,
  type ProgramBlock,
} from 'logic-mesh';
import { cloneWidget, type Block, type Widget } from './Block';
import { useEngine } from './Engine';
import { widgetBlockDescs } from './Widgets';

const { command } = useEngine();

/** The editor metadata this editor saves with a block. */
export type BlockEditor = {
  /** Widget identity, for ExternalIn/ExternalOut blocks placed as widgets. */
  widget?: Widget;
};

/** A program as this editor saves and loads it. */
export type EditorProgram = Program<BlockEditor>;

type EditorBlock = ProgramBlock<BlockEditor>;

export function save(ops: {
  name: string;
  desc?: string;
  nodes: Node[];
  edges: Edge[];
}): EditorProgram {
  const program: EditorProgram = {
    version: PROGRAM_FORMAT_VERSION,
    name: ops.name,
    description: ops.desc,
    blocks: {},
    links: {},
  };

  ops.nodes.forEach((node) => {
    const blockRef = node.data as { value: Block };
    const data = blockRef.value;
    const { desc } = data;

    const curProgram: EditorBlock = {
      name: desc.name,
      lib: desc.lib,
      ver: desc.ver,
      position: { x: node.position.x, y: node.position.y },
    };
    program.blocks[node.id] = curProgram;
    if (data.label) {
      curProgram.label = data.label;
    }
    if (data.widget) {
      const widget = cloneWidget(data.widget);
      // An empty driven-config map is dropped from the save format.
      if (widget.configSources && !Object.keys(widget.configSources).length) {
        delete widget.configSources;
      }
      curProgram.editor = { widget };
    }

    Object.entries(data.inputs).forEach(([name, input]) => {
      if (input.value != null) {
        curProgram.inputs = curProgram.inputs || {};
        curProgram.inputs[name] = {
          value: input.value,
          isConnected: !!input.isConnected,
        };
      }
    });

    Object.entries(data.outputs).forEach(([name, output]) => {
      if (output.value != null) {
        curProgram.outputs = curProgram.outputs || {};
        curProgram.outputs[name] = { value: output.value, isConnected: false };
      }
    });
  });

  ops.edges.forEach((edge) => {
    program.links[
      (edge.data as { id?: string } | undefined)?.id ?? crypto.randomUUID()
    ] = {
      sourceBlockPinName: edge.sourceHandle ?? '',
      targetBlockPinName: edge.targetHandle ?? '',
      sourceBlockUuid: edge.source,
      targetBlockUuid: edge.target,
    };
  });

  return program;
}

/**
 * A block as a save from before format versions wrote it: the position
 * keyed `positions`, the widget beside the block's fields rather than in
 * its editor metadata, and pins whose value or connected flag may be
 * missing.
 */
type LegacyBlock = Omit<EditorBlock, 'inputs' | 'outputs'> & {
  positions?: Position;
  widget?: Widget;
  inputs?: Record<string, Partial<PinValue>>;
  outputs?: Record<string, Partial<PinValue>>;
};

/** A save from before format versions, see {@link LegacyBlock}. */
type LegacyProgram = Omit<EditorProgram, 'version' | 'blocks' | 'links'> & {
  version?: undefined;
  blocks?: Record<string, LegacyBlock>;
  links?: EditorProgram['links'];
};

/**
 * Bring a saved program to the current format, in place, so the same
 * object is valid for both node building and `pushToEngine`.
 *
 * A save from before format versions — one without `version`, written
 * by logic-mesh 1.x or an earlier build of this editor — moves its
 * `positions` to `position` and a block's `widget` into its editor
 * metadata, and gets the pin fields it left out. A program from a newer
 * format is rejected before anything is built from it.
 */
function upgradeFormat(saved: unknown): EditorProgram {
  const program = saved as EditorProgram | LegacyProgram;
  if (program.version !== undefined) {
    if (program.version > PROGRAM_FORMAT_VERSION) {
      throw new Error(
        `Program format version ${program.version} is newer than this editor reads (${PROGRAM_FORMAT_VERSION})`,
      );
    }
    return program;
  }

  const blocks = (program.blocks ??= {});
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
  program.links ??= {};
  return Object.assign(program, {
    version: PROGRAM_FORMAT_VERSION,
  }) as EditorProgram;
}

// Widget names from the legacy JS-block UI library (lib 'ui'), split by
// data direction: input widgets became ExternalIn, display widgets
// became ExternalOut. Derived from the palette descriptors, whose
// `direction` field is the single source of truth — a hand-maintained
// list here would silently diverge when a widget is added.
function legacyWidgetKinds(direction: 'in' | 'out'): Set<string> {
  return new Set(
    widgetBlockDescs
      .filter((desc) => desc.widget.direction === direction)
      .map((desc) => desc.widget.kind),
  );
}
const LEGACY_INPUT_WIDGETS = legacyWidgetKinds('in');
const LEGACY_DISPLAY_WIDGETS = legacyWidgetKinds('out');

// Extracts the widget config from a legacy block's config pins.
function legacyConfig(
  name: string,
  block: EditorBlock,
): Record<string, unknown> | undefined {
  const pin = (p: string) => block.inputs?.[p]?.value;
  const out = block.outputs?.['out']?.value;
  switch (name) {
    case 'Slider':
      return {
        value: out ?? 0,
        min: pin('min') ?? 0,
        max: pin('max') ?? 100,
        step: pin('step') ?? 1,
      };
    case 'Input':
      return { value: out ?? '' };
    case 'Checkbox':
      return { value: out ?? false };
    case 'ComboBox':
      return {
        items: pin('in') ?? '',
        ...(out !== undefined ? { value: out } : {}),
      };
    case 'MultiChart':
      return { series: [{ label: pin('labelA') ?? '' }] };
    case 'Led':
      return { label: pin('label') ?? '', color: pin('color') ?? '#3ecf6b' };
    case 'Bar':
      return {
        min: pin('min') ?? 0,
        max: pin('max') ?? 100,
        label: pin('label') ?? '',
      };
    case 'Display':
      return { unit: pin('unit') ?? '', label: pin('label') ?? '' };
    default:
      return undefined;
  }
}

// Former config pins per legacy widget, mapped to the widget config
// key each one set. A link into one of these becomes a `configSources`
// entry driven by a synthesized plain ExternalOut (see
// `migrateLegacyUiBlocks`). Only flat, top-level config keys can be
// driven this way — `useWidgetConfig` overrides by key, so nested
// values (MultiChart per-series labels) are not representable and
// their links drop with a warning instead.
const LEGACY_CONFIG_PIN_KEYS: Record<string, Record<string, string>> = {
  Slider: { min: 'min', max: 'max', step: 'step' },
  // The old ComboBox's `in` pin carried the CSV items list, not the
  // selected value.
  ComboBox: { in: 'items' },
  Led: { label: 'label', color: 'color' },
  Bar: { min: 'min', max: 'max', label: 'label' },
  Display: { unit: 'unit', label: 'label' },
};

// Former value-drive pins on input widgets: the linked value used to
// write the widget's own output, racing the operator. The new model
// expresses program-driven values through `valueSource` (the widget
// tracks a plain ExternalOut's publishes as feedback, and operator
// interaction wins while it lasts), so a link into one of these
// becomes the widget's valueSource.
const LEGACY_VALUE_PINS: Record<string, string> = {
  Slider: 'in',
  Input: 'in',
  Checkbox: 'in',
};

// The old MultiChart's extra data pins and the series slot each fed;
// pin 'a' is series 0 and stays on the widget block's own 'in'.
const LEGACY_MULTICHART_SERIES: Record<string, number> = { b: 1, c: 2, d: 3 };
// Per-slot label pins, indexed like the series.
const LEGACY_MULTICHART_LABELS = ['labelA', 'labelB', 'labelC', 'labelD'];

/** What a legacy block was before migration rewrote it in place. */
interface LegacyBlockInfo {
  direction: 'in' | 'out';
  kind: string;
  /** `Kind 'label'` (or just the kind) for warning messages. */
  who: string;
  /** The pre-migration input pins, kept for label literals. */
  oldInputs: NonNullable<EditorBlock['inputs']>;
}

/** What a migration pass did, for surfacing to the user on load. */
export interface MigrationReport {
  /** Links converted to config sources / value sources / series. */
  converted: string[];
  /** Links that could not be mapped and were dropped. */
  dropped: string[];
}

/**
 * Migrate legacy UI JS-block entries (lib 'ui') in a saved program to
 * the ExternalIn/ExternalOut + widget-metadata shape, in place, so the
 * same program object is valid for both node building and
 * `pushToEngine`.
 *
 * Links are converted rather than dropped wherever the new model has
 * an equivalent:
 * - the MultiChart 'a' pin is retargeted to 'in';
 * - a link into a former config pin (a Bar's `min`, a Led's `label`,
 *   the ComboBox items) is rerouted through a synthesized plain
 *   ExternalOut whose address is recorded in the widget's
 *   `configSources`;
 * - a link into an input widget's former value-drive pin becomes the
 *   widget's `valueSource`, again via a synthesized ExternalOut;
 * - the old MultiChart data pins b/c/d each become an extra series
 *   entry (`series[i].address`) fed by a synthesized ExternalOut,
 *   keeping the corresponding labelB/C/D literal as the series label.
 *
 * What has no equivalent — links into the per-series label pins
 * (nested config keys are not drivable) and links from removed output
 * pins — is dropped, and every drop is reported so the load can warn.
 */
function migrateLegacyUiBlocks(program: EditorProgram): MigrationReport {
  const report: MigrationReport = { converted: [], dropped: [] };
  if (!program.blocks) return report;

  const migrated = new Map<string, LegacyBlockInfo>();
  for (const [uuid, block] of Object.entries(program.blocks)) {
    if (block.lib !== 'ui' || block.editor?.widget) continue;
    const name = block.name ?? '';
    const direction = LEGACY_INPUT_WIDGETS.has(name)
      ? 'in'
      : LEGACY_DISPLAY_WIDGETS.has(name)
        ? 'out'
        : undefined;
    if (!direction) continue;
    migrated.set(uuid, {
      direction,
      kind: name,
      who: block.label ? `${name} '${block.label}'` : name,
      oldInputs: block.inputs ?? {},
    });

    const config = legacyConfig(name, block);
    // The display widget's value pin; MultiChart's first series was 'a'.
    const inPin =
      direction === 'out'
        ? (block.inputs?.[name === 'MultiChart' ? 'a' : 'in'] ?? undefined)
        : undefined;
    const outPin = direction === 'in' ? block.outputs?.['out'] : undefined;

    block.editor = { ...block.editor, widget: { kind: name, config } };
    block.name = direction === 'in' ? 'ExternalIn' : 'ExternalOut';
    block.lib = 'core';
    const inputs: NonNullable<EditorBlock['inputs']> = {
      connector: { value: 'ui', isConnected: false },
      address: { value: uuid, isConnected: false },
    };
    if (inPin) {
      inputs['in'] = { value: inPin.value, isConnected: inPin.isConnected };
    }
    block.inputs = inputs;
    if (outPin && outPin.value != null) {
      block.outputs = { out: { value: outPin.value, isConnected: false } };
    } else {
      // Remove the key outright rather than assigning `undefined`: the
      // migrated object goes to `loadProgram` as-is (no JSON round-trip
      // to drop the key), and serde on the wasm side rejects a present
      // `outputs` key holding `undefined` — an absent key falls back to
      // the empty default.
      delete block.outputs;
    }
  }

  if (!migrated.size || !program.links) return report;

  // One synthesized ExternalOut per (widget, former pin), reused if
  // several links fed the same pin; keyed for lookup, counted per
  // widget so the synthesized nodes stagger on the canvas instead of
  // stacking.
  const synthesized = new Map<string, string>();
  const synthCount = new Map<string, number>();

  // Input widgets that still feed downstream blocks. The old model
  // echoed a driven value straight through to those links; after a
  // value link converts to `valueSource` feedback, downstream only
  // sees user gestures — worth a warning (see the valueSource branch).
  const feedsDownstream = new Set<string>();
  for (const link of Object.values(program.links)) {
    if (
      migrated.get(link.sourceBlockUuid)?.direction === 'in' &&
      link.sourceBlockPinName === 'out'
    ) {
      feedsDownstream.add(link.sourceBlockUuid);
    }
  }

  // Creates (or reuses) the plain ExternalOut block that carries a
  // converted link's live value to `widgetUuid`, returning its address.
  // A fresh block uuid doubles as the address, the same scheme widget
  // blocks use for their own addresses.
  function synthExternalOut(
    widgetUuid: string,
    pin: string,
    label: string,
  ): { address: string; reused: boolean } {
    const key = `${widgetUuid}:${pin}`;
    const existing = synthesized.get(key);
    if (existing) {
      // Several links fed the same former pin. They now share one
      // ExternalOut input, so whichever writes last wins — the old
      // model had the same race, but make the sharing visible.
      const who = migrated.get(widgetUuid)?.who ?? widgetUuid;
      report.dropped.push(
        `${who}: several links fed former pin '${pin}'; they now share one ExternalOut input and the last write wins`,
      );
      return { address: existing, reused: true };
    }

    const uuid = crypto.randomUUID();
    const n = synthCount.get(widgetUuid) ?? 0;
    synthCount.set(widgetUuid, n + 1);
    const widgetPos = program.blocks[widgetUuid]?.position;
    program.blocks[uuid] = {
      name: 'ExternalOut',
      lib: 'core',
      label,
      // To the widget's left, staggered per synthesized block, so the
      // rerouted links stay short and nothing piles up at the origin.
      position: {
        x: (widgetPos?.x ?? 0) - 240,
        y: (widgetPos?.y ?? 0) + 90 * n,
      },
      inputs: {
        in: { value: null, isConnected: true },
        connector: { value: 'ui', isConnected: false },
        address: { value: uuid, isConnected: false },
      },
    };
    synthesized.set(key, uuid);
    return { address: uuid, reused: false };
  }

  for (const [linkId, link] of Object.entries(program.links)) {
    // A migrated block's only surviving output pin is 'out' (ExternalIn
    // out, or the ExternalOut publish echo).
    const src = migrated.get(link.sourceBlockUuid);
    if (src && link.sourceBlockPinName !== 'out') {
      report.dropped.push(
        `${src.who}: link from removed output pin '${link.sourceBlockPinName}' could not be migrated`,
      );
      delete program.links[linkId];
      continue;
    }

    const tgt = migrated.get(link.targetBlockUuid);
    if (!tgt) continue;
    const pin = link.targetBlockPinName;
    const widget = program.blocks[link.targetBlockUuid]?.editor?.widget;

    if (tgt.kind === 'MultiChart') {
      if (pin === 'a') {
        link.targetBlockPinName = 'in';
        continue;
      }
      const idx = LEGACY_MULTICHART_SERIES[pin];
      if (idx !== undefined && widget) {
        const { address, reused } = synthExternalOut(
          link.targetBlockUuid,
          pin,
          `${tgt.who} series ${idx + 1}`,
        );
        link.targetBlockUuid = address;
        link.targetBlockPinName = 'in';
        // A reused block was already configured and reported; the
        // shared-input warning above is all this link adds.
        if (reused) continue;
        // Extend the series array up to this slot; the label literal
        // the old per-series label pin held carries over.
        const config = (widget.config ??= {});
        const series = Array.isArray(config.series)
          ? (config.series as Record<string, unknown>[])
          : [{ label: '' }];
        while (series.length <= idx) series.push({ label: '' });
        const oldLabel = tgt.oldInputs[LEGACY_MULTICHART_LABELS[idx]]?.value;
        series[idx] = {
          label: typeof oldLabel === 'string' ? oldLabel : '',
          address,
        };
        config.series = series;
        report.converted.push(
          `${tgt.who}: data pin '${pin}' became series ${idx + 1}, fed by a new ExternalOut`,
        );
        continue;
      }
      report.dropped.push(
        `${tgt.who}: link into removed pin '${pin}' could not be migrated`,
      );
      delete program.links[linkId];
      continue;
    }

    // The display widget's value pin survives as ExternalOut 'in'.
    if (tgt.direction === 'out' && pin === 'in') continue;

    if (widget && pin === LEGACY_VALUE_PINS[tgt.kind]) {
      const widgetUuid = link.targetBlockUuid;
      const { address, reused } = synthExternalOut(
        widgetUuid,
        pin,
        `${tgt.who} value`,
      );
      link.targetBlockUuid = address;
      link.targetBlockPinName = 'in';
      // A reused block was already configured and reported; the
      // shared-input warning above is all this link adds.
      if (reused) continue;
      widget.valueSource = address;
      report.converted.push(
        `${tgt.who}: value link now tracked as feedback (valueSource) via a new ExternalOut`,
      );
      if (feedsDownstream.has(widgetUuid)) {
        // The old block echoed driven values out of its 'out' pin;
        // valueSource is feedback-only, so that flow is severed.
        report.dropped.push(
          `${tgt.who}: driven values no longer flow to its downstream links — the widget's output now carries only user changes`,
        );
      }
      continue;
    }

    const configKey = LEGACY_CONFIG_PIN_KEYS[tgt.kind]?.[pin];
    if (widget && configKey) {
      const { address, reused } = synthExternalOut(
        link.targetBlockUuid,
        pin,
        `${tgt.who} ${configKey}`,
      );
      link.targetBlockUuid = address;
      link.targetBlockPinName = 'in';
      // A reused block was already configured and reported; the
      // shared-input warning above is all this link adds.
      if (reused) continue;
      widget.configSources = {
        ...widget.configSources,
        [configKey]: address,
      };
      report.converted.push(
        `${tgt.who}: config pin '${pin}' now driven live (configSources.${configKey}) via a new ExternalOut`,
      );
      continue;
    }

    report.dropped.push(
      `${tgt.who}: link into removed pin '${pin}' could not be migrated`,
    );
    delete program.links[linkId];
  }

  return report;
}

/**
 * Build the UI node/edge structures from program data, synchronously.
 *
 * Done as a separate step from `pushToEngine` so the caller can register
 * the resulting blocks (e.g., into a `blockInstances` map keyed by id)
 * BEFORE any wasm engine commands fire. Otherwise the first
 * change-of-value notifications from the engine — which arrive while the
 * engine is still processing the load — get dropped because the watcher
 * callback can't find the block id yet.
 *
 * Brings a save from before format versions to the current format and
 * migrates legacy UI JS-block programs, both in place, and returns the
 * (migrated) `program` to pass to `pushToEngine`. A program from a newer
 * format throws before anything is built. The returned `migration`
 * report says what the UI-block migration converted and what it had to
 * drop — callers surface it to the user (a load must not silently lose
 * links).
 */
export function prepare(saved: unknown): {
  program: EditorProgram;
  nodes: Node[];
  edges: Edge[];
  migration: MigrationReport;
} {
  const program = upgradeFormat(saved);
  const migration = migrateLegacyUiBlocks(program);
  const nodes: Node[] = [];
  const edges: Edge[] = [];

  for (const blockUuid in program.blocks) {
    const block = program.blocks[blockUuid];
    nodes.push({
      id: blockUuid,
      type: 'custom',
      position: { x: block.position?.x ?? 0, y: block.position?.y ?? 0 },
      data: {
        name: block.name ?? '',
        lib: block.lib ?? '',
        label: block.label ?? '',
        widget: block.editor?.widget,
        inputs: block.inputs ?? {},
        outputs: block.outputs ?? {},
      },
    });
  }

  for (const linkId in program.links) {
    const link = program.links[linkId];
    edges.push({
      id: linkId,
      source: link.sourceBlockUuid,
      target: link.targetBlockUuid,
      sourceHandle: link.sourceBlockPinName,
      targetHandle: link.targetBlockPinName,
    });
  }

  return { program, nodes, edges, migration };
}

/**
 * Push the program into the wasm engine in a single atomic call.
 *
 * The Rust engine accepts a full `Program` (blocks + links + pin values
 * + UI and editor metadata) via `loadProgram` and handles scheduling, wiring, and
 * value writes internally — no JS-side `addBlock` / `createLink` /
 * `writeBlockInput` chain. Call AFTER `prepare()` + registering the
 * block instances so change-of-value notifications fired during the
 * load have a registered destination.
 */
export async function pushToEngine(program: EditorProgram): Promise<void> {
  await command.loadProgram(program);
}

/**
 * Convenience wrapper: `prepare` + `pushToEngine`. Used by callers that
 * don't need the prepare/register/push split — but if you have a
 * `blockInstances` map keyed by id and you care about not losing the
 * first round of COV notifications, call `prepare` and `pushToEngine`
 * separately and register your instances between them.
 */
export async function load(saved: unknown): Promise<{
  program: EditorProgram;
  nodes: Node[];
  edges: Edge[];
  migration: MigrationReport;
}> {
  const prepared = prepare(saved);
  await pushToEngine(prepared.program);
  return prepared;
}
