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
		if (data.label) {
			program.blocks[node.id].label = data.label;
		}

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

/**
 * Build the UI node/edge structures from program data, synchronously.
 *
 * Done as a separate step from `pushToEngine` so the caller can register
 * the resulting blocks (e.g., into a `blockInstances` map keyed by id)
 * BEFORE any wasm engine commands fire. Otherwise the first
 * change-of-value notifications from the engine — which arrive while the
 * engine is still processing the load — get dropped because the watcher
 * callback can't find the block id yet.
 */
export function prepare(program: Program): { nodes: Node[]; edges: Edge[] } {
	const nodes: Node[] = [];
	const edges: Edge[] = [];

	for (const blockUuid in program.blocks) {
		const block = program.blocks[blockUuid];
		nodes.push({
			id: blockUuid,
			type: 'custom',
			position: { x: block.positions?.x ?? 0, y: block.positions?.y ?? 0 },
			data: {
				name: block.name ?? '',
				lib: block.lib ?? '',
				label: block.label ?? '',
				inputs: block.inputs ?? {},
				outputs: block.outputs ?? {},
			},
		});
	}

	for (const linkId in program.links) {
		const link = program.links[linkId];
		edges.push({
			id: linkId,
			source: link.sourceBlockUuid,
			target: link.targetBlockUuid,
			sourceHandle: link.sourceBlockPinName,
			targetHandle: link.targetBlockPinName,
		});
	}

	return { nodes, edges };
}

/**
 * Push the program into the wasm engine: addBlock, createLink, write
 * initial values. Call AFTER `prepare()` + registering the block
 * instances so change-of-value notifications fired during the load have
 * a registered destination.
 */
export async function pushToEngine(program: Program): Promise<void> {
	for (const blockUuid in program.blocks) {
		const block = program.blocks[blockUuid];
		const id = await command.addBlock(block.name, blockUuid, block.lib);
		if (blockUuid !== id) {
			throw new Error(`Block uuid mismatch: ${blockUuid} !== ${id}`);
		}
	}

	for (const linkId in program.links) {
		const link = program.links[linkId];
		await command.createLink(
			link.sourceBlockUuid,
			link.targetBlockUuid,
			link.sourceBlockPinName,
			link.targetBlockPinName
		);
	}

	for (const blockUuid in program.blocks) {
		const block = program.blocks[blockUuid];

		for (const [name, input] of Object.entries(block.inputs ?? {})) {
			if (hasInitialValue(input.value)) {
				await command.writeBlockInput(blockUuid, name, input.value);
			}
		}

		for (const [name, output] of Object.entries(block.outputs ?? {})) {
			if (hasInitialValue(output.value)) {
				await command.writeBlockOutput(blockUuid, name, output.value);
			}
		}
	}
}

/**
 * Convenience wrapper: `prepare` + `pushToEngine`. Used by callers that
 * don't need the prepare/register/push split — but if you have a
 * `blockInstances` map keyed by id and you care about not losing the
 * first round of COV notifications, call `prepare` and `pushToEngine`
 * separately and register your instances between them.
 */
export async function load(program: Program): Promise<{ nodes: Node[]; edges: Edge[] }> {
	const { nodes, edges } = prepare(program);
	await pushToEngine(program);
	return { nodes, edges };
}

/**
 * Treat empty objects/arrays as placeholders, not real initial values.
 *
 * Examples typically annotate a computed output pin with `value: {}` purely
 * as a UI hint that the pin exists. Loading that verbatim would push an
 * empty `Dict` into the wasm input/output, which then propagates through
 * connected links to Number-typed downstream pins and faults their drain
 * (Cannot convert Dict to Number). It also overwrites any value the block
 * has already computed for that pin.
 */
function hasInitialValue(val: unknown): boolean {
	if (val == null) return false;
	if (typeof val !== 'object') return true;
	if (Array.isArray(val)) return val.length > 0;
	return Object.keys(val as object).length > 0;
}
