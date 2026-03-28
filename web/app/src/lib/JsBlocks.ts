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

export function registerBlocks(engine: BlocksEngine) {
	InputBlock.register(engine);
	CheckboxBlock.register(engine);
	GaugeBlock.register(engine);
	ChartBlock.register(engine);
}
