import { BlockDesc, BlocksEngine, JsBlock } from 'logic-mesh'

async function passThroughExecute(inputs: unknown[]): Promise<unknown[]> {
	return [inputs[0]]
}

function passThroughFactory(): (inputs: unknown[]) => Promise<unknown[]> {
	return passThroughExecute
}

/**
 * A Text input block
 */
const InputBlock = {
	desc: {
		name: 'Input',
		dis: 'Input',
		lib: 'ui',
		ver: '0.0.1',
		category: 'UI',
		doc: 'An input box',
		implementation: 'external',
		inputs: [
			{
				name: 'in',
				kind: 'str',
			},
		],
		outputs: [
			{
				name: 'out',
				kind: 'str',
			},
		],
	} satisfies BlockDesc,
	executor: passThroughFactory,
} satisfies JsBlock

/**
 * A Checkbox block
 */
const CheckboxBlock = {
	desc: {
		name: 'Checkbox',
		dis: 'Checkbox',
		lib: 'ui',
		ver: '0.0.1',
		category: 'UI',
		doc: 'A checkbox',
		implementation: 'external',
		inputs: [
			{
				name: 'in',
				kind: 'bool',
			},
		],
		outputs: [
			{
				name: 'out',
				kind: 'bool',
			},
		],
	} satisfies BlockDesc,
	executor: passThroughFactory,
} satisfies JsBlock

/**
 * Block that renders a gauge for the input data
 */
const GaugeBlock = {
	desc: {
		name: 'Gauge',
		dis: 'Gauge',
		lib: 'ui',
		ver: '0.0.1',
		category: 'UI',
		doc: 'A gauge',
		implementation: 'external',
		inputs: [
			{
				name: 'in',
				kind: 'number',
			},
		],
		outputs: [
			{
				name: 'out',
				kind: 'number',
			},
		],
	} satisfies BlockDesc,
	executor: passThroughFactory,
} satisfies JsBlock

/**
 * Block that renders a chart for the input data
 */
const ChartBlock = {
	desc: {
		name: 'Chart',
		dis: 'Chart',
		lib: 'ui',
		ver: '0.0.1',
		category: 'UI',
		doc: 'A line chart',
		implementation: 'external',
		inputs: [
			{
				name: 'in',
				kind: 'str',
			},
		],
		outputs: [],
	} satisfies BlockDesc,
} satisfies JsBlock

export function registerBlocks(engine: BlocksEngine) {
	engine.registerBlock(InputBlock.desc, InputBlock.executor)
	engine.registerBlock(CheckboxBlock.desc, CheckboxBlock.executor)
	engine.registerBlock(GaugeBlock.desc, GaugeBlock.executor)
	engine.registerBlock(ChartBlock.desc)
}
