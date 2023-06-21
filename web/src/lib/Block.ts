
/**
 * Describe a block that is available in block library.
 */
export interface BlockDesc {
	name: string;
	lib: string;
	category: string;
	doc: string;

	inputs: BlockPin[];
	outputs: BlockPin[];
}

/**
 * Create a block instance from a block description.
 * @param id The unique id of the block instance.
 * @param desc The block description.
 * @returns A block instance.
 */
export function blockInstance(id: string, desc: BlockDesc): Block {
	function toObj(pins: BlockPin[]) {
		return pins.reduce((acc, pin) => {
			acc[pin.name] = {...pin, value: undefined}
			return acc
		 }, {} as {
			[key: string]: BlockPin
		})
	}

	return {
		id,
		desc,
		inputs: toObj(desc.inputs),
		outputs: toObj(desc.outputs),
	}
}

/**
 * A block instance.
 **/
export interface Block { 
	id: string

	desc: BlockDesc

	inputs: {
		[key: string]: BlockPin
	}

	outputs: {
		[key: string]: BlockPin
	}
}

/**
 * A block pin.
 */
export interface BlockPin {
	name: string;
	kind: string;
	value: unknown
}