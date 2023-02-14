<script setup lang="ts">
import { ref } from 'vue';
import * as logic from 'logic-mesh'

import { MarkerType, VueFlow } from '@vue-flow/core'
import { MiniMap } from '@vue-flow/minimap'
import { Controls } from '@vue-flow/controls'
import { Background } from '@vue-flow/background'

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';
import Button from 'primevue/Button';

import BlockNode from './components/BlockNode.vue'

const blocks = logic.listBlocks()
const engine = logic.initEngine()

const elements = ref([
	// Nodes
	// An input node, specified by using `type: 'input'`
	{ id: '1', type: 'custom', label: 'Node 1', position: { x: 250, y: 5 }, data: { name: "aaa" } },

	// An output node, specified by using `type: 'output'`
	{ id: '4', type: 'custom', label: 'Node 4', position: { x: 400, y: 200 } },

	{ id: 'e1-4', source: '1', target: '4', targetHandleId: 'a', type: 'step', markerEnd: MarkerType.Arrow },
	{ id: 'e1-41', source: '1', target: '4', targetHandleId: 'b', type: 'step', markerEnd: MarkerType.Arrow },
])

engine.run().then(() => console.log("Running here"))


</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="10">
			<div v-for="block of blocks" :key="block.name">
				<Button :label="block.name" :title="block.doc" size="small" class="m-1" style="width: 10em">
				</Button>
			</div>
		</SplitterPanel>
		<SplitterPanel :size="90">
			<VueFlow v-model="elements" class="customnodeflow" :default-edge-options="{ type: 'smoothstep' }"
				:min-zoom="1" :max-zoom="4" :elevate-edges-on-select="true" :apply-default="true">
				<Background pattern-color="#aaa" gap="8" />

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
@import '@vue-flow/core/dist/style.css';
@import '@vue-flow/core/dist/theme-default.css';
@import '@vue-flow/minimap/dist/style.css';
@import '@vue-flow/controls/dist/style.css';


@import 'primevue/resources/themes/tailwind-light/theme.css';
@import 'primevue/resources/primevue.css';
@import 'primeicons/primeicons.css';
@import 'primeflex/primeflex.css';


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