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
}
