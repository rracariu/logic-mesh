import { GraphEdge, GraphNode, VueFlowStore, useVueFlow } from '@vue-flow/core'
import { Ref, ref } from 'vue'
import { Block, blockInstance } from './Block'
import { useEngine } from './Engine'
import { BlockDesc } from 'logic-mesh'

export const currentBlock = ref<GraphNode<any, any, string>>()
export const currentLink = ref<GraphEdge>()

export const blockInstances = new Map<string, Ref<Block>>()
export let state: VueFlowStore

const { command } = useEngine()

export async function addBlock(desc: BlockDesc): Promise<Block> {
	const id = await command.addBlock(desc.name)

	const block = ref(blockInstance(id, desc))

	let position = { x: 250, y: 5 }

	if (currentBlock.value) {
		const x = currentBlock.value.position.x
		const y = currentBlock.value.position.y

		position = { x: x ? x + 200 : 250, y: y ? y + 10 : 5 }
	}

	state.addNodes({
		id,
		type: 'custom',
		label: desc.name,
		position,
		data: block,
	})

	blockInstances.set(id, block)
	currentBlock.value = state.findNode(id)

	return block.value
}

export function removeBlock(id: string) {
	state.removeNodes([id])
	command.removeBlock(id)
	blockInstances.delete(id)
}

export function useFlowModel() {
	if (!state) {
		state = useVueFlow()
	}

	const {
		edges,
		nodes,
		removeEdges,
		addNodes,
		addEdges,
		findNode,
		removeNodes,
		deleteKeyCode,
	} = state

	return {
		edges,
		nodes,
		removeEdges,
		addNodes,
		addEdges,
		findNode,
		removeNodes,
		deleteKeyCode,
	}
}
