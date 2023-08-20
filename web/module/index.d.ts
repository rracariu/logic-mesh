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
	 * The block function
	 * @param inputs The block inputs that have been set
	 * @returns The block outputs that have to be set
	 */
	function: (inputs: unknown[]) => Promise<unknown[]>
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
