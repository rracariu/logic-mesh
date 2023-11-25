export { BlocksEngine, initEngine, EngineCommand } from './logic_mesh'

/**
 * The kind of the block pin.
 */
export type Kind =
	| 'null'
	| 'remove'
	| 'marker'
	| 'na'
	| 'bool'
	| 'number'
	| 'str'
	| 'uri'
	| 'ref'
	| 'symbol'
	| 'date'
	| 'time'
	| 'dateTime'
	| 'coord'
	| 'xstr'
	| 'list'
	| 'dict'
	| 'grid'

/**
 * A block that is implemented in JS
 */
export type JsBlock = {
	/**
	 * The block description
	 */
	desc: BlockDesc

	/**
	 * An optional block factory function that returns a function that is called when the block is executed.
	 * @returns The execute function that is called when the block is executed.
	 */
	executor?: () => (
		inputs: (unknown | undefined)[]
	) => Promise<(unknown | undefined)[]>
}

/**
 * A block pin.
 *
 * Pins are the inputs and outputs of a block.
 */
export interface BlockPin {
	/**
	 * The pin name
	 */
	name: string

	/**
	 * The pin kind
	 * Must be a valid haystack type kind.
	 * See https://project-haystack.org/doc/docHaystack/Kinds
	 */
	kind: Kind

	/**
	 * The pin value
	 * Value is a Haystack value encoded as JSON.
	 */
	value?: unknown

	/**
	 * True if the pin is connected to another pin.
	 */
	isConnected?: boolean
}

/**
 * Describe a block that is available in block library.
 */
export interface BlockDesc {
	/**
	 * The block name
	 */
	name: string

	/**
	 * The block display name
	 */
	dis: string

	/**
	 * The block library name
	 */
	lib: string

	/**
	 * The block library version
	 */
	ver: string

	/**
	 * The block category
	 */
	category: string

	/**
	 * The block documentation
	 */
	doc: string

	/**
	 * The block implementation
	 */
	implementation: 'native' | 'external'

	/**
	 * The block inputs
	 */
	inputs: BlockPin[]

	/**
	 * The block outputs
	 */
	outputs: BlockPin[]

	/**
	 * The block run condition.
	 *
	 * If not set, the block will be executed when any of its inputs change.
	 * Otherwise, the block will execute regularly according to the run condition.
	 *
	 * Default: 'change'
	 */
	runCondition?: 'change' | 'always'
}

/**
 * Notification on a block change
 */
export interface BlockNotification {
	/**
	 * The block id
	 */
	id: string

	/**
	 * The changes
	 */
	changes: {
		/**
		 * The block pin name
		 */
		name: string
		/**
		 * The block pin source
		 */
		source: string
		/**
		 * The value that was changed on this pin
		 */
		value: {}
	}[]
}

export interface LinkData {
	/**
	 * The link id
	 */
	id?: string

	/**
	 * The link source block pin name
	 */
	sourceBlockPinName: string

	/**
	 * The link source block uuid
	 */
	sourceBlockUuid: string

	/**
	 * The link target block pin name
	 */
	targetBlockPinName: string

	/**
	 * The link target block uuid
	 */
	targetBlockUuid: string
}

/**
 * Describes a program that would be loaded in the engine.
 *
 * The program is a set of blocks and links between them.
 * The program and the blocks have meta data that would be used in the editor
 * for example to position the blocks.
 */
export interface Program {
	/**
	 * The program name
	 */
	name: string

	/**
	 * Optional program description
	 */
	description?: string

	/**
	 * Blocks used in the program
	 */
	blocks: {
		[blockUuid: string]: {
			name: string
			lib: string

			positions: {
				x: number
				y: number
			}

			inputs?: {
				[pinName: string]: { value?: unknown; isConnected?: boolean }
			}

			outputs?: {
				[pinName: string]: { value?: unknown; isConnected?: boolean }
			}
		}
	}

	/**
	 * Links between blocks
	 */
	links: {
		[linkUuid: string]: LinkData
	}
}
