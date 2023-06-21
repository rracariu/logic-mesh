<script setup lang="ts">
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { Connection, VueFlow, useVueFlow } from '@vue-flow/core';
import { MiniMap } from '@vue-flow/minimap';

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';

import BlockList from './components/BlockList.vue';
import BlockNode from './components/BlockNode.vue';
import { BlockDesc, blockInstance } from './lib/Block';
import { Notification, command, blocks, startWatch } from './lib/Engine';

const { nodes, edges, addNodes, addEdges, findNode } = useVueFlow()

startWatch((notification: Notification) => {
	const block = findNode(notification.id)

	if (!block || !notification.changes.length) {
		return
	}

	notification.changes.forEach((change) => {
		if (change.source === 'input') {
			edges.value.filter((edge) => edge.target === block.id && edge.targetHandle === change.name)
				.forEach((edge) => {
					edge.animated = true
				})
		}

		const pins = change.source === 'input' ? block.data?.inputs : block.data?.outputs
		pins[change.name].value = change.value
	})
})

const addBlock = async (desc: BlockDesc) => {
	const id = await command.addBlock(desc.name)
	if (id) {
		addNodes(
			{ id, type: 'custom', label: desc.name, position: { x: 250, y: 5 }, data: blockInstance(id, desc) }
		)
	}
}

const onBlockClick = async (id: string) => {
	const data = await command.inspectBlock(id)
	console.table([...(data.outputs as Map<string, unknown>).values()])
}

const onConnect = async (conn: Connection) => {
	const id = await command.createLink(conn.source, conn.target, conn.sourceHandle ?? '', conn.targetHandle ?? '')
	if (id) {
		addEdges({ ...conn, id })
	}
}

</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="18">
			<BlockList :blocks="blocks" @add-block="addBlock" />

		</SplitterPanel>
		<SplitterPanel :size="82">
			<VueFlow @connect="onConnect" :default-edge-options="{ type: 'smoothstep' }" :min-zoom="1" :max-zoom="4"
				:elevate-edges-on-select="true" :apply-default="true" auto-connect>
				<Background pattern-color="#aaa" :gap="8" />

				<template #node-custom="{ data }">
					<BlockNode :data="data" @click="onBlockClick(data.id)" />
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


.vue-flow__node-custom {
	font-size: medium;
	border: 1px solid #777;
	padding: 3px;
	border-radius: 3px;
	background: linear-gradient(180deg, var(--surface-50) 0%, var(--surface-100) 100%);
	display: flex;
	flex-direction: column;
	justify-content: space-between;
	align-items: center;
	gap: 3px;
	max-width: 250px;
	min-width: 6em;
}
</style>