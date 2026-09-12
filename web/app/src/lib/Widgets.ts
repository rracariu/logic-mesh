import type { BlockDesc } from 'logic-mesh';

/**
 * Palette entry for a UI widget. Placing one creates an engine
 * ExternalIn (input widgets) or ExternalOut (display widgets) block
 * wired to the 'ui' connector, with the widget identity kept in the
 * node/program data.
 */
export interface WidgetBlockDesc extends BlockDesc {
  widget: {
    kind: string;
    direction: 'in' | 'out';
    defaultConfig?: Record<string, unknown>;
  };
}

export function isWidgetDesc(desc: BlockDesc): desc is WidgetBlockDesc {
  return 'widget' in desc;
}

function widgetDesc(
  kind: string,
  direction: 'in' | 'out',
  doc: string,
  defaultConfig?: Record<string, unknown>,
): WidgetBlockDesc {
  return {
    name: kind,
    dis: kind,
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc,
    implementation: 'external',
    inputs: [],
    outputs: [],
    widget: { kind, direction, defaultConfig },
  };
}

export const widgetBlockDescs: WidgetBlockDesc[] = [
  widgetDesc(
    'Slider',
    'in',
    'A horizontal slider for numeric range selection',
    {
      value: 0,
      min: 0,
      max: 100,
      step: 1,
    },
  ),
  widgetDesc('Input', 'in', 'An input box', { value: '' }),
  widgetDesc('Checkbox', 'in', 'A checkbox', { value: false }),
  widgetDesc(
    'Button',
    'in',
    'A push button, outputs true on press, false on release',
  ),
  widgetDesc('ComboBox', 'in', 'A combo box with CSV items and custom entry', {
    items: '',
  }),
  widgetDesc('Table', 'in', 'A key-value table for creating dictionaries'),
  widgetDesc('Gauge', 'out', 'A gauge'),
  widgetDesc('Chart', 'out', 'A line chart'),
  widgetDesc(
    'MultiChart',
    'out',
    'A line chart; series 0 is the in pin, extra series subscribe to other ExternalOut addresses',
    { series: [{ label: '' }] },
  ),
  widgetDesc('Label', 'out', 'A read-only display for any value'),
  widgetDesc(
    'Led',
    'out',
    'Indicator lamp; lit when in is true. Optional label and color (hex/css).',
    { label: '', color: '#3ecf6b' },
  ),
  widgetDesc(
    'Bar',
    'out',
    'Vertical level bar for damper/valve position or any bounded quantity.',
    { min: 0, max: 100, label: '' },
  ),
  widgetDesc(
    'Display',
    'out',
    'Numeric readout with optional label and unit suffix.',
    {
      unit: '',
      label: '',
    },
  ),
];
