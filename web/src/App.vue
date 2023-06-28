<script setup lang="ts">
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { Connection, EdgeMouseEvent, NodeMouseEvent, OnConnectStartParams, VueFlow, useVueFlow } from '@vue-flow/core';
import { MiniMap } from '@vue-flow/minimap';

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';

import BlockList from './components/BlockList.vue';
import BlockNode from './components/BlockNode.vue';
import { Block, BlockDesc, blockInstance } from './lib/Block';
import { Notification, command, blocks, startWatch } from './lib/Engine';
import { Ref, onMounted, ref } from 'vue';
import { currentBlock, currentLink } from './lib/Model'

const { edges, removeEdges, addNodes, findNode, removeNodes, deleteKeyCode } = useVueFlow()
const blockMap = new Map<string, Ref<Block>>()
deleteKeyCode.value = null

onMounted(() => {
	onkeydown = (event: KeyboardEvent) => {
		if (event.key === 'Delete') {
			if (currentBlock.value) {
				removeNodes([currentBlock.value.data.id])
				command.removeBlock(currentBlock.value.data.id)
				blockMap.delete(currentBlock.value.data.id)

				currentBlock.value = undefined
			} else if (currentLink.value) {
				removeEdges([currentLink.value.id])
				command.removeLink(currentLink.value.data.id)
				currentLink.value = undefined
			}
		}
	}
})

startWatch((notification: Notification) => {
	const block = blockMap.get(notification.id)

	if (!block || !notification.changes.length) {
		return
	}

	notification.changes.forEach((change) => {
		if (change.source === 'input') {
			edges.value.filter((edge) => edge.target === block.value.id && edge.targetHandle === change.name)
				.forEach((edge) => {
					edge.animated = true
				})
		}

		const pins = change.source === 'input' ? block.value.inputs : block.value.outputs
		pins[change.name].value = change.value
	})
})

const addBlock = async (desc: BlockDesc) => {
	const id = await command.addBlock(desc.name)
	if (id) {
		const data = ref(blockInstance(id, desc))

		let position = { x: 250, y: 5 }

		if (currentBlock.value) {
			const x = currentBlock.value.position.x
			const y = currentBlock.value.position.y

			position = { x: x ? x + 200 : 250, y: y ? y + 10 : 5 }
		}

		addNodes(
			{ id, type: 'custom', label: desc.name, position, data }
		)
		blockMap.set(id, data)
		currentBlock.value = findNode(id)
	}
}

let connSource: OnConnectStartParams | undefined

const onConnect = (conn: Connection) => {
	if (!connSource) {
		return
	}

	if (connSource.handleType === 'target') {
		conn = { source: conn.target, target: conn.source, sourceHandle: conn.targetHandle, targetHandle: conn.sourceHandle }
	}

	connSource = undefined

	return command.createLink(conn.source, conn.target, conn.sourceHandle ?? '', conn.targetHandle ?? '').then((data) => {
		if (data) {
			const link = edges.value.find((edge) => edge.target === conn.target
				&& edge.source === conn.source
				&& edge.sourceHandle === conn.sourceHandle
				&& edge.targetHandle === conn.targetHandle)

			if (link) {
				link.data = data
			}
		}
	})
}

const onConnectStart = (conn: OnConnectStartParams) => {
	connSource = conn
}

const onBlockClick = (event: NodeMouseEvent) => {
	currentLink.value = undefined
	currentBlock.value = event.node
}

const onEdgeClick = (event: EdgeMouseEvent) => {
	currentBlock.value = undefined
	currentLink.value = event.edge
}

</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="18">
			<BlockList :blocks="blocks" @add-block="addBlock" />

		</SplitterPanel>
		<SplitterPanel :size="82">
			<VueFlow @connect="onConnect" @connect-start="onConnectStart" @node-click="onBlockClick"
				@edge-click="onEdgeClick" :default-edge-options="{ type: 'smoothstep' }" :min-zoom="1" :max-zoom="4"
				:elevate-edges-on-select="true" :apply-default="true" auto-connect>
				<Background pattern-color="#aaa" :gap="8" />

				<template #node-custom="{ data }">
					<BlockNode :data="data" />
				</template>

				<Controls />
				<MiniMap></MiniMap>
			</VueFlow>
		</SplitterPanel>
	</Splitter>
</template>

<style>
@import 'primevue/resources/themes/tailwind-light/theme.css';
@import 'primevue/resources/primevue.css';
@import 'primeicons/primeicons.css';
@import 'primeflex/primeflex.css';

@import '@vue-flow/core/dist/style.css';
@import '@vue-flow/core/dist/theme-default.css';
@import '@vue-flow/minimap/dist/style.css';
@import '@vue-flow/controls/dist/style.css';

html {
	font-size: 14px;
}

.vue-flow__node-custom {
	font-size: smaller;
	border: 1px solid var(--surface-300);
	padding: 0px;
	border-radius: 5px;
	background: linear-gradient(180deg, var(--surface-50) 0%, var(--surface-100) 100%);
	display: flex;
	flex-direction: column;
	justify-content: space-between;
	align-items: center;
	gap: 3px;
	max-width: 250px;
	min-width: 8em;
}
</style>