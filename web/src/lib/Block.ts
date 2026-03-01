import type { BlockDesc, BlockPin } from 'logic-mesh';

/**
 * A block instance.
 */
export interface Block {
	id: string;
	desc: BlockDesc;
	inputs: { [key: string]: BlockPin };
	outputs: { [key: string]: BlockPin };
}

/**
 * Create a block instance from a block description.
 */
export function blockInstance(id: string, desc: BlockDesc): Block {
	function toObj(pins: BlockPin[]) {
		return pins.reduce(
			(acc, pin) => {
				acc[pin.name] = { ...pin, value: undefined, isConnected: false };
				return acc;
			},
			{} as { [key: string]: BlockPin }
		);
	}

	return {
		id,
		desc,
		inputs: toObj(desc.inputs),
		outputs: toObj(desc.outputs),
	};
}
