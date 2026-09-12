import type { BlockDesc, BlockPin } from 'logic-mesh';

/**
 * UI widget identity carried by ExternalIn/ExternalOut blocks that
 * were placed as widgets. `kind` picks the component; `config` holds
 * widget-local settings (min/max/label/...).
 */
export interface Widget {
  kind: string;
  config?: Record<string, unknown>;
  /**
   * Config key → address of a plain ExternalOut block. Values the
   * engine publishes to that address override the literal `config`
   * value for the key at runtime.
   */
  configSources?: Record<string, string>;
  /**
   * Address of a plain ExternalOut block whose published values an
   * input widget tracks as feedback. User interaction still pushes
   * through the widget's own address and wins while interacting.
   */
  valueSource?: string;
}

// Plain-data deep clone for widget configs. Hand-rolled rather than
// `structuredClone` because configs are routinely read off `$state`
// proxies, which the structured clone algorithm rejects.
function deepCloneConfig<T>(value: T): T {
  if (Array.isArray(value)) {
    return value.map(deepCloneConfig) as T;
  }
  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    for (const [key, val] of Object.entries(value)) {
      out[key] = deepCloneConfig(val);
    }
    return out as T;
  }
  return value;
}

/**
 * The single clone path for every place a widget is copied — palette
 * placement, clipboard, paste, program save and load. Deep-clones
 * `config` (nested arrays/objects included, e.g. MultiChart's `series`
 * array and its element objects), shallow-copies the flat
 * `configSources` map, and carries `valueSource` — so no widget
 * instance ever aliases another's (or the palette default's) config.
 */
export function cloneWidget(widget: Widget): Widget {
  return {
    kind: widget.kind,
    config: widget.config ? deepCloneConfig(widget.config) : undefined,
    configSources: widget.configSources
      ? { ...widget.configSources }
      : undefined,
    valueSource: widget.valueSource,
  };
}

/**
 * A block instance.
 */
export interface Block {
  id: string;
  desc: BlockDesc;
  /** Widget identity, present only on widget-backed external blocks. */
  widget?: Widget;
  /** Optional user label shown next to the block-type name. */
  label: string;
  inputs: { [key: string]: BlockPin };
  outputs: { [key: string]: BlockPin };
  /** Operational state from the engine. Drives fault-ring rendering. */
  state: 'running' | 'fault' | 'disabled' | 'terminated';
  /** Reason associated with `state === 'fault'`, if any. */
  faultReason?: string;
}

/**
 * Create a block instance from a block description.
 */
export function blockInstance(id: string, desc: BlockDesc): Block {
  function toObj(pins: BlockPin[]) {
    return pins.reduce(
      (acc, pin) => {
        acc[pin.name] = { ...pin, value: undefined, isConnected: false };
        return acc;
      },
      {} as { [key: string]: BlockPin },
    );
  }

  return {
    id,
    desc,
    label: '',
    inputs: toObj(desc.inputs),
    outputs: toObj(desc.outputs),
    state: 'running',
    faultReason: undefined,
  };
}
