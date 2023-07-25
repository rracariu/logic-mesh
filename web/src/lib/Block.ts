import { BlockDesc, BlockPin } from 'logic-mesh'

/**
 * Create a block instance from a block description.
 * @param id The unique id of the block instance.
 * @param desc The block description.
 * @returns A block instance.
 */
export function blockInstance(id: string, desc: BlockDesc): Block {
	function toObj(pins: BlockPin[]) {
		return pins.reduce(
			(acc, pin) => {
				acc[pin.name] = { ...pin, value: undefined }
				return acc
			},
			{} as {
				[key: string]: BlockPin
			}
		)
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
