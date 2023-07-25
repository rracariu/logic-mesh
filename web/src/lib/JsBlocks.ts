import { BlockDesc, BlocksEngine, JsBlock } from 'logic-mesh'

/**
 * Defines a block that is implemented in JS
 */
const InputBlock = {
	desc: {
		name: 'Input',
		dis: 'Input',
		lib: 'ui',
		ver: '0.0.1',
		category: 'UI',
		doc: 'An input box ',
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

export function registerBlocks(engine: BlocksEngine) {
	engine.registerBlock(InputBlock.desc, InputBlock.function)
}
