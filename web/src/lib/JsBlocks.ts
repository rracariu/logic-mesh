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
	function: async (inputs: unknown[]) => {
		return ['']
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
	function: async (inputs: unknown[]) => {
		return ['']
	},
} satisfies JsBlock

export function registerBlocks(engine: BlocksEngine) {
	engine.registerBlock(InputBlock.desc, InputBlock.function)
	engine.registerBlock(ChartBlock.desc, ChartBlock.function)
}
