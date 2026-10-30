import type { Block, Widget } from './Block';
import { UI_CONNECTOR_NAME } from './UiConnector';

/**
 * An address a widget reads besides its own: a `ui` connector address
 * that a plain ExternalOut block publishes to. These bindings are not
 * links, so the canvas draws no edge for them.
 */
export interface WidgetSource {
  /**
   * What the published values drive: `value` for a `valueSource`, the
   * config key for a `configSources` entry, or a MultiChart series'
   * label.
   */
  role: string;
  address: string;
}

/**
 * Every source a widget subscribes to, in the order the widget reads
 * them: its `valueSource`, its `configSources` entries, then the
 * MultiChart series that name an address. An address equal to the
 * widget's own (`ownAddress`) is not a source.
 */
export function widgetSources(
  widget: Widget | undefined,
  ownAddress?: string,
): WidgetSource[] {
  if (!widget) return [];
  const sources: WidgetSource[] = [];
  const add = (role: string, address: unknown) => {
    if (typeof address === 'string' && address && address !== ownAddress) {
      sources.push({ role, address });
    }
  };

  add('value', widget.valueSource);
  for (const [key, address] of Object.entries(widget.configSources ?? {})) {
    add(key, address);
  }
  // Mirrors MultiChartBlock: any series with an address subscribes to
  // it, and an unlabelled one is named by its slot.
  const series = widget.kind === 'MultiChart' ? widget.config?.series : null;
  if (Array.isArray(series)) {
    series.forEach((entry, i) => {
      const s = (entry ?? {}) as Record<string, unknown>;
      const label =
        s.label != null && String(s.label).length > 0
          ? String(s.label)
          : `series ${i + 1}`;
      add(label, s.address);
    });
  }
  return sources;
}

/**
 * The ExternalOut block that publishes to `address` on the `ui`
 * connector, or `undefined` when none does — then the widget never
 * receives a value from that source.
 */
export function findPublisher(
  address: string,
  blocks: Iterable<Block>,
): Block | undefined {
  for (const block of blocks) {
    if (
      block.desc.name === 'ExternalOut' &&
      block.inputs['connector']?.value === UI_CONNECTOR_NAME &&
      block.inputs['address']?.value === address
    ) {
      return block;
    }
  }
  return undefined;
}

/** The name a source block goes by on the canvas. */
export function blockDisplayName(block: Block): string {
  if (block.label) return block.label;
  const configLabel = block.widget?.config?.label;
  if (typeof configLabel === 'string' && configLabel) return configLabel;
  return block.widget?.kind ?? block.desc.name;
}
