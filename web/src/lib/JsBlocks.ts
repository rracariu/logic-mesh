import { BlockDesc, BlocksEngine, JsBlock } from 'logic-mesh'

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
	function: async (_inputs: unknown[]) => {
		return [false]
	},
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
	engine.registerBlock(InputBlock.desc)
	engine.registerBlock(CheckboxBlock.desc, CheckboxBlock.function)
	engine.registerBlock(ChartBlock.desc)
}
