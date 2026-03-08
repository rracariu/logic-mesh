import type { Edge, Node } from '@xyflow/svelte';
import type { BlockDesc } from 'logic-mesh';
import { blockInstance, type Block } from './Block';
import { useEngine } from './Engine';

const { command } = useEngine();

// Registry of live block instances keyed by node id
export const blockInstances = new Map<string, { value: Block }>();

/**
 * Central reactive model for the flow.
 *
 * Uses a class so that properties can be freely reassigned from any module.
 * @xyflow/svelte v1 requires $state.raw for nodes/edges (immutable array replacements).
 */
class FlowModel {
	nodes = $state.raw<Node[]>([]);
	edges = $state.raw<Edge[]>([]);
	currentBlock = $state<Node | undefined>(undefined);
	currentEdge = $state<Edge | undefined>(undefined);

	async addBlock(desc: BlockDesc): Promise<Block> {
		const id = await command.addBlock(desc.name, undefined, desc.lib);
		// Wrap in $state so pin value mutations are reactive
		const blockValue = $state(blockInstance(id, desc));
		const block = { value: blockValue };

		const position = this.currentBlock
			? {
					x: (this.currentBlock.position.x || 0) + 200,
					y: (this.currentBlock.position.y || 0) + 10,
				}
			: { x: 250, y: 5 };

		this.nodes = [...this.nodes, { id, type: 'custom', position, data: block }];
		blockInstances.set(id, block);
		this.currentBlock = this.nodes.find((n) => n.id === id);

		return block.value;
	}

	removeBlock(id: string) {
		this.nodes = this.nodes.filter((n) => n.id !== id);
		this.edges = this.edges.filter((e) => e.source !== id && e.target !== id);
		command.removeBlock(id);
		blockInstances.delete(id);
	}

	removeEdgeById(id: string) {
		this.edges = this.edges.filter((e) => e.id !== id);
	}

	clearAll() {
		this.nodes = [];
		this.edges = [];
		blockInstances.clear();
		this.currentBlock = undefined;
		this.currentEdge = undefined;
	}
}

export const model = new FlowModel();
