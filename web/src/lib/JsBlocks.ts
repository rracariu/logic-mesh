import { BlocksEngine } from 'logic-mesh'
import { BlockDesc } from './Block'

export type JsBlock = {
	desc: BlockDesc
	function: (inputs: unknown[]) => Promise<unknown[]>
}

/**
 * Defines a block that is implemented in JS
 */
const SampleJsBlock = {
	desc: {
		name: 'JsTest',
		dis: 'JS block',
		lib: 'test',
		ver: '0.0.1',
		category: 'test',
		doc: 'Tests the blocks JS interface',
		variant: 'external',
		inputs: [{ name: 'in', kind: 'Number' }],
		outputs: [{ name: 'out', kind: 'Number' }],
	} satisfies BlockDesc,
	function: async (inputs: unknown[]) => {
		console.log(
			`Test block called with ${inputs.length} inputs, first is ${inputs[0]}`
		)
		return [(inputs[0] as number) ** 2]
	},
} satisfies JsBlock

export function registerBlocks(engine: BlocksEngine) {
	engine.registerBlock(SampleJsBlock.desc, SampleJsBlock.function)
}
