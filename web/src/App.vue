<script setup lang="ts">
import * as logic from 'logic-mesh'

import { Connection, VueFlow } from '@vue-flow/core'
import { MiniMap } from '@vue-flow/minimap'
import { Controls } from '@vue-flow/controls'
import { Background } from '@vue-flow/background'

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';

import BlockNode from './components/BlockNode.vue'
import BlockList from './components/BlockList.vue';
import { Block } from './lib/Block';
import { BlockNodesModel } from './model/BlockNodesModel';

const engine = logic.initEngine()

const blocks = engine.listBlocks()
const command = engine.engineCommand()

const addBlock = async (block: Block) => {
	const id = await command.addBlock(block.name)

	if (id) {
		BlockNodesModel.value.push(
			{ id, type: 'custom', label: block.name, position: { x: 250, y: 5 }, data: { ...block, id } }
		)
	}
}

const onBlockClick = async (id: string) => {
	const data = await command.inspectBlock(id)
	console.table([...(data.outputs as Map<string, unknown>).values()])
}

const onConnect = async (conn: Connection) => {
	const id = await command.createLink(conn.source, conn.target, conn.sourceHandle ?? '', conn.targetHandle ?? '')
}

// Start the engine
engine.run()

</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="18">
			<BlockList :blocks="blocks" @add-block="addBlock" />

		</SplitterPanel>
		<SplitterPanel :size="82">
			<VueFlow v-model="BlockNodesModel" @connect="onConnect" :default-edge-options="{ type: 'smoothstep' }"
				:min-zoom="1" :max-zoom="4" :elevate-edges-on-select="true" :apply-default="true" auto-connect>
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