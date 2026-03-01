import type { Edge, Node } from '@xyflow/svelte';
import type { BlockPin, Program } from 'logic-mesh';
import type { Block } from './Block';
import { useEngine } from './Engine';

const { command } = useEngine();

export function save(ops: { name: string; desc?: string; nodes: Node[]; edges: Edge[] }): Program {
	const program: Program = {
		name: ops.name,
		description: ops.desc,
	} as Program;

	ops.nodes.forEach((node) => {
		const blockRef = node.data as { value: Block };
		const data = blockRef.value;
		const { desc } = data;

		program.blocks = program.blocks || {};
		program.blocks[node.id] = {
			name: desc.name,
			lib: desc.lib,
			positions: { x: node.position.x, y: node.position.y },
		};

		const curProgram = program.blocks[node.id];

		Object.entries(data.inputs).forEach(([name, input]) => {
			if (input.value != null) {
				curProgram.inputs = curProgram.inputs || {};
				curProgram.inputs[name] = { value: input.value, isConnected: input.isConnected };
			}
		});

		Object.entries(data.outputs).forEach(([name, output]) => {
			if (output.value != null) {
				curProgram.outputs = curProgram.outputs || {};
				curProgram.outputs[name] = { value: output.value };
			}
		});
	});

	ops.edges.forEach((edge) => {
		program.links = program.links || {};
		program.links[(edge.data as { id?: string } | undefined)?.id ?? crypto.randomUUID()] = {
			sourceBlockPinName: edge.sourceHandle ?? '',
			targetBlockPinName: edge.targetHandle ?? '',
			sourceBlockUuid: edge.source,
			targetBlockUuid: edge.target,
		};
	});

	return program;
}

export async function load(program: Program): Promise<{ nodes: Node[]; edges: Edge[] }> {
	const nodes: Node[] = [];
	const edges: Edge[] = [];

	for (const blockUuid in program.blocks) {
		const block = program.blocks[blockUuid];
		const id = await command.addBlock(block.name, blockUuid, block.lib);
		if (blockUuid !== id) {
			throw new Error(`Block uuid mismatch: ${blockUuid} !== ${id}`);
		}

		nodes.push({
			id: blockUuid,
			type: 'custom',
			position: { x: block.positions?.x ?? 0, y: block.positions?.y ?? 0 },
			data: {
				name: block.name ?? '',
				lib: block.lib ?? '',
				inputs: block.inputs ?? {},
				outputs: block.outputs ?? {},
			},
		});
	}

	for (const linkId in program.links) {
		const link = program.links[linkId];
		await command.createLink(
			link.sourceBlockUuid,
			link.targetBlockUuid,
			link.sourceBlockPinName,
			link.targetBlockPinName
		);

		edges.push({
			id: linkId,
			source: link.sourceBlockUuid,
			target: link.targetBlockUuid,
			sourceHandle: link.sourceBlockPinName,
			targetHandle: link.targetBlockPinName,
		});
	}

	for (const blockUuid in program.blocks) {
		const block = program.blocks[blockUuid];

		for (const [name, input] of Object.entries(block.inputs ?? {})) {
			if (input.value != null) {
				await command.writeBlockInput(blockUuid, name, input.value);
			}
		}

		for (const [name, output] of Object.entries(block.outputs ?? {})) {
			if (output.value != null) {
				await command.writeBlockOutput(blockUuid, name, output.value);
			}
		}
	}

	return { nodes, edges };
}
