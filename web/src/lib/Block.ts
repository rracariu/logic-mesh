export interface Block {
	id: string;
	name: string;
	lib: string;
	category: string;
	doc: string;

	inputs: BlockPin[];
	outputs: BlockPin[];
}

export interface BlockPin {
	name: string;
	kind: string;
	value: unknown
}