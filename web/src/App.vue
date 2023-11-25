<script setup lang="ts">
import { Background } from '@vue-flow/background';
import { Controls } from '@vue-flow/controls';
import { Connection, EdgeMouseEvent, OnConnectStartParams, Panel, VueFlow, useEdge, useVueFlow } from '@vue-flow/core';
import { MiniMap } from '@vue-flow/minimap';

import { useClipboard } from '@vueuse/core';

import Splitter from 'primevue/splitter';
import SplitterPanel from 'primevue/splitterpanel';
import PrimePanel from 'primevue/panel';
import Toast from 'primevue/toast';
import { useToast } from "primevue/usetoast";

import { BlockPin, Program } from 'logic-mesh';
import ProgressBar from 'primevue/progressbar';
import Textarea from 'primevue/textarea';
import { Ref, onMounted, ref } from 'vue';
import BlockList from './components/BlockList.vue';
import BlockTemplate from './components/BlockNode.vue';
import Toolbar from './components/ToolBar.vue';
import { Block, blockInstance } from './lib/Block';
import type { BlockDesc, BlockNotification, EngineCommand, LinkData } from 'logic-mesh';
import { currentBlock, currentLink } from './lib/Model';
import { load, save } from './lib/Program';
import { useEngine } from './lib/Engine';


const toast = useToast();

const { edges, nodes, removeEdges, addNodes, addEdges, findNode, removeNodes, deleteKeyCode } = useVueFlow()
const blockMap = new Map<string, Ref<Block>>()
deleteKeyCode.value = null

const { engine, blocks, command, startWatch } = useEngine()
let engineRunning = false

onMounted(() => {
	if (!engineRunning) {

		engine.run()

		startWatch((notification: BlockNotification) => {
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
	}

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

const addBlock = (desc: BlockDesc) => {
	command.addBlock(desc.name).then(id => {
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
	})
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

	return command.createLink(conn.source, conn.target, conn.sourceHandle ?? '', conn.targetHandle ?? '').then((data: LinkData) => {
		if (data) {
			const link = edges.value.find((edge) => edge.target === conn.target
				&& edge.source === conn.source
				&& edge.sourceHandle === conn.sourceHandle
				&& edge.targetHandle === conn.targetHandle)

			if (link) {
				const sourceBlock = blockMap.get(conn.source)
				if (sourceBlock) {
					const input = sourceBlock.value.inputs[conn.sourceHandle ?? '']
					if (input) input.isConnected = true
					const output = sourceBlock.value.outputs[conn.sourceHandle ?? '']
					if (output) output.isConnected = true
				}

				const targetBlock = blockMap.get(conn.target)
				if (targetBlock) {
					const input = targetBlock.value.inputs[conn.targetHandle ?? '']
					if (input) input.isConnected = true
					const output = targetBlock.value.outputs[conn.targetHandle ?? '']
					if (output) output.isConnected = true
				}

				link.data = data
			}
		}
	})
}

const onConnectStart = (conn: OnConnectStartParams) => {
	connSource = conn
}

const onBlockClick = (event: any) => {
	currentLink.value = undefined
	currentBlock.value = event.node
}

const onEdgeClick = (event: EdgeMouseEvent) => {
	currentBlock.value = undefined
	currentLink.value = event.edge
}

async function onReset() {
	await command.resetEngine();
	blockMap.clear();

	currentBlock.value = undefined;
	currentLink.value = undefined;

	removeEdges(edges.value.map((edge) => edge.id));
	removeNodes(nodes.value.map((node) => node.id));
}

function onCopy() {
	const program = save({ name: 'test', nodes: nodes.value, edges: edges.value })
	const { copy } = useClipboard()
	copy(JSON.stringify(program, (_, value) => {
		if (typeof value === 'number') {
			return parseFloat(value.toFixed(2))
		}
		return value
	}))

	toast.add({ severity: 'success', summary: 'Copy', detail: 'Program copied...', life: 3000 });
}

function onPaste() {
	onReset().then(async () => {
		const clipText = await navigator.clipboard
			.readText();
		const program = JSON.parse(clipText);
		await loadProgram(program);
	}).catch(
		(err) => {
			toast.add({ severity: 'error', summary: 'Paste', detail: err, life: 3000 });
		},
	)
}

function onLoad(program: Program) {
	onReset().then(async () => {
		await loadProgram(program);
	}).catch(
		(err) => {
			toast.add({ severity: 'error', summary: 'Load', detail: err, life: 3000 });
		},
	)
}

async function loadProgram(program: any) {
	let { nodes, edges } = await load(program);

	nodes = nodes.map((node) => {
		const desc = blocks.find((block) => block.name === node.data.name) ?? node.data;
		const block = ref(blockInstance(node.id, desc));
		blockMap.set(node.id, block);

		for (const [name, e] of Object.entries(node.data.inputs ?? {})) {
			const input = e as BlockPin
			block.value.inputs[name].value = input.value;
			block.value.inputs[name].isConnected = input.isConnected;
		}

		for (const [name, e] of Object.entries(node.data.outputs ?? {})) {
			const output = e as BlockPin
			block.value.outputs[name].value = output.value;
			block.value.outputs[name].isConnected = output.isConnected;
		}

		node.data = block;
		return node;
	});

	addNodes(nodes);
	addEdges(edges);

	toast.add({ severity: 'success', summary: 'Load', detail: 'Program loaded...', life: 3000 });
}

// AI

const clientId = crypto.randomUUID()
const prompt = ref('')
const processing = ref(false)

async function assistantPrompt() {
	if (!prompt.value) {
		return
	}

	processing.value = true

	const response = await fetch('/api/blocks/builder', {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: JSON.stringify({ prompt: prompt.value, clientId })
	})

	const json = await response.json()

	let program = json.messages.data[0].content[0].text.value as string

	program = program.replace('```json\n', '');
	program = program.replace('```', '');
	program = program.replace(/\/\/[\s\S]+/g, '');

	console.log(program)

	await onReset()
	await loadProgram(JSON.parse(program))

	processing.value = false
}

</script>

<template>
	<Splitter style="height: 97vh">
		<SplitterPanel :size="18">
			<BlockList :blocks="blocks" @add-block="addBlock" />
		</SplitterPanel>
		<SplitterPanel :size="82">
			<Toast />
			<VueFlow @connect="onConnect" @connect-start="onConnectStart" @node-click="onBlockClick"
				@edge-click="onEdgeClick" :default-edge-options="{ type: 'smoothstep' }" :min-zoom="1" :max-zoom="4"
				:elevate-edges-on-select="true" :apply-default="true" auto-connect>
				<Background pattern-color="#aaa" :gap="8" />

				<template #node-custom="{ data }">
					<BlockTemplate :data="data" />
				</template>

				<Controls />
				<MiniMap></MiniMap>
				<Panel position="bottom-center" class="controls">
					<Toolbar @reset="onReset" @copy="onCopy" @paste="onPaste" @load="onLoad" style="min-width: 30em;" />
				</Panel>
				<Panel position="top-left" class="controls">
					<PrimePanel header="Assistant" toggleable :collapsed="true">
						<ProgressBar v-if="processing" mode="indeterminate" style="height: 3px;" />
						<Textarea v-model="prompt" placeholder="Type assistant instructions..." :disabled="processing"
							rows="5" cols="30" @keydown.stop.prevent.enter="assistantPrompt()" />
					</PrimePanel>
				</Panel>
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
	max-width: 400px;
	min-width: 8em;
}
</style>