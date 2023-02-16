<script setup lang="ts">
import { ref } from 'vue';
import * as logic from 'logic-mesh'

import { VueFlow } from '@vue-flow/core'
import { MiniMap } from '@vue-flow/minimap'
import { Controls } from '@vue-flow/controls'
import { Background } from '@vue-flow/background'

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';
import Button from 'primevue/Button';

import BlockNode from './components/BlockNode.vue'

const engine = logic.initEngine()

const blocks = engine.listBlocks()
const command = engine.engineCommand()

const elements = ref([] as any[])

const addBlock = async (block: any) => {
	const id = await command.addBlock(block.name)

	if (id) {
		elements.value.push(
			{ id: id, type: 'custom', label: block.name, position: { x: 250, y: 5 }, data: { id, ...block } }
		)
	}
}

const onBlockClick = async (id: string) => {
	console.log(await command.inspectBlock(id))
}

engine.run().then(() => console.log("Running here"))


</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="10">
			<div v-for="block of blocks" :key="block.name">
				<Button :label="block.name" :title="block.doc" @click="addBlock(block)" size="small" class="m-1"
					style="width: 10em">
				</Button>
			</div>
		</SplitterPanel>
		<SplitterPanel :size="90">
			<VueFlow v-model="elements" :default-edge-options="{ type: 'smoothstep' }" :min-zoom="1" :max-zoom="4"
				:elevate-edges-on-select="true" :apply-default="true" auto-connect>
				<Background pattern-color="#aaa" :gap="8" />

				<template #node-custom="{ data }">
					<BlockNode :data="data" @out-click="onBlockClick" />
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
	border: 1px solid #777;
	padding: 10px;
	border-radius: 7px;
	background: whitesmoke;
	display: flex;
	flex-direction: column;
	justify-content: space-between;
	align-items: center;
	gap: 10px;
	max-width: 250px
}
</style>