import { defineBlock, type BlocksEngine } from 'logic-mesh';
import { z } from 'zod';

const InputBlock = defineBlock({
  desc: {
    name: 'Input',
    dis: 'Input',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'An input box',
  },
  inputs: [['in', z.string()]] as const,
  outputs: [['out', z.string()]] as const,
  execute: async ([input]) => [input],
});

const CheckboxBlock = defineBlock({
  desc: {
    name: 'Checkbox',
    dis: 'Checkbox',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A checkbox',
  },
  inputs: [['in', z.boolean()]] as const,
  outputs: [['out', z.boolean()]] as const,
  execute: async ([input]) => [input],
});

const GaugeBlock = defineBlock({
  desc: {
    name: 'Gauge',
    dis: 'Gauge',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A gauge',
  },
  inputs: [['in', z.number()]] as const,
  outputs: [['out', z.number()]] as const,
  execute: async ([input]) => [input],
});

const ChartBlock = defineBlock({
  desc: {
    name: 'Chart',
    dis: 'Chart',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A line chart',
  },
  inputs: [['in', z.string()]] as const,
  outputs: [],
});

const ButtonBlock = defineBlock({
  desc: {
    name: 'Button',
    dis: 'Button',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A push button, outputs true on press, false on release',
  },
  inputs: [],
  outputs: [['out', z.boolean()]] as const,
  execute: async () => [false],
});

const ComboBoxBlock = defineBlock({
  desc: {
    name: 'ComboBox',
    dis: 'ComboBox',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A combo box with CSV items input and custom entry',
  },
  inputs: [['in', z.string()]] as const,
  outputs: [['out', z.string()]] as const,
  execute: async ([input]) => [input],
});

const TableBlock = defineBlock({
  desc: {
    name: 'Table',
    dis: 'Table',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A key-value table for creating dictionaries',
  },
  inputs: [],
  outputs: [['out', z.string()]] as const,
  execute: async () => ['{}'],
});

const LabelBlock = defineBlock({
  desc: {
    name: 'Label',
    dis: 'Label',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A read-only display for any value',
  },
  inputs: [['in', z.string()]] as const,
  outputs: [],
});

const SliderBlock = defineBlock({
  desc: {
    name: 'Slider',
    dis: 'Slider',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'A horizontal slider for numeric range selection',
  },
  inputs: [
    ['in', z.number()],
    ['min', z.number()],
    ['max', z.number()],
    ['step', z.number()],
  ] as const,
  outputs: [['out', z.number()]] as const,
  execute: async ([input]) => [input],
});

const LedBlock = defineBlock({
  desc: {
    name: 'Led',
    dis: 'Led',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'Indicator lamp; lit when in is true. Optional label and color (hex/css).',
  },
  inputs: [
    ['in', z.boolean()],
    ['label', z.string()],
    ['color', z.string()],
  ] as const,
  outputs: [],
});

const BarBlock = defineBlock({
  desc: {
    name: 'Bar',
    dis: 'Bar',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'Vertical level bar for damper/valve position or any 0-100 quantity.',
  },
  inputs: [
    ['in', z.number()],
    ['min', z.number()],
    ['max', z.number()],
    ['label', z.string()],
  ] as const,
  outputs: [],
});

const DisplayBlock = defineBlock({
  desc: {
    name: 'Display',
    dis: 'Display',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'Numeric readout with optional label and unit suffix.',
  },
  inputs: [
    ['in', z.number()],
    ['unit', z.string()],
    ['label', z.string()],
  ] as const,
  outputs: [],
});

const MultiChartBlock = defineBlock({
  desc: {
    name: 'MultiChart',
    dis: 'MultiChart',
    lib: 'ui',
    ver: '0.0.1',
    category: 'UI',
    doc: 'Line chart with up to 4 overlaid numeric series (a, b, c, d) and per-series label inputs.',
  },
  inputs: [
    ['a', z.number()],
    ['b', z.number()],
    ['c', z.number()],
    ['d', z.number()],
    ['labelA', z.string()],
    ['labelB', z.string()],
    ['labelC', z.string()],
    ['labelD', z.string()],
  ] as const,
  outputs: [],
});

export function registerBlocks(engine: BlocksEngine) {
  InputBlock.register(engine);
  CheckboxBlock.register(engine);
  GaugeBlock.register(engine);
  ChartBlock.register(engine);
  ButtonBlock.register(engine);
  ComboBoxBlock.register(engine);
  TableBlock.register(engine);
  LabelBlock.register(engine);
  SliderBlock.register(engine);
  LedBlock.register(engine);
  BarBlock.register(engine);
  DisplayBlock.register(engine);
  MultiChartBlock.register(engine);
}
