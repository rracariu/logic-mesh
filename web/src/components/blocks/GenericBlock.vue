<script setup lang="ts">
import { Connection, Handle, Position, } from '@vue-flow/core';

import { computed } from 'vue';
import { Block } from '../../lib/Block';
import { currentBlock } from '../../lib/Model';

const props = defineProps<{ data: Block }>()

const handlePos = (index: number) => `top: ${index * 1.5 + 3.0}em`

const inputPins = computed(() => {
	const ins = props.data.inputs

	if (!Object.keys(ins).every((k) => k.match(/^[a-zA-z]+[0-9]+$/))) {
		return ins
	}

	const entries = Object.entries(ins)

	let lastConnected = 0
	for (let i = 0; i < entries.length; i++) {
		if (!!entries[i][1].isConnected) {
			lastConnected = i > 0 ? i + 1 : i
		}
	}

	const res: Block['inputs'] = {}
	for (let i = 0; i < Math.min(lastConnected + 2, entries.length); i++) {
		res[entries[i][0]] = entries[i][1]
	}

	return res
})

const blockStyle = computed(() => {
	let css = `width: 100%; height: ${Object.keys(inputPins.value).length * 1.3 + 3.0}em; `

	if (currentBlock.value?.data.id === props.data.id) {
		css += 'box-shadow: 2px 2px 7px 3px var(--surface-200);'
	}

	return css
})

const validConnection = (conn: Connection) => {
	return conn.source !== conn.target
}

const format = (value: object) => {
	return typeof value === 'number' ? Intl.NumberFormat().format(value) : value.toString().slice(0, 8)
}

</script>

<template>
	<div :style="blockStyle">
		<div class="header">
			<label :title="data.desc.doc"> {{ data.desc.name }} </label>
		</div>
		<Handle v-for="(input, name, index) in inputPins" :key="name" :id="input.name"
			:is-valid-connection="validConnection" type="target" :position="Position.Left" :style="handlePos(index)"
			class="block-input">
			{{ name }} {{ input.value != null ? `${format(input.value)}` : '' }}
		</Handle>


		<Handle v-for="(output, name, index) in data.outputs" :key="name" :id="output.name"
			:is-valid-connection="validConnection" type="source" :position="Position.Right" class="block-output"
			:style="handlePos(index)">
			{{ name }} {{ output.value != null ? `${format(output.value)}` : '' }}
		</Handle>
	</div>
</template>

<style scoped>
.header {
	border-bottom: 1px solid var(--surface-200);
	border-radius: 0px !important;
	width: 100%;
	text-align: center;
}

[class*="block-"] {
	font-size: x-small;
	padding: 1px;
	display: inline-table;
	border-radius: 10% !important;
	min-width: 5em !important;
	white-space: nowrap;
	overflow: hidden;
	text-overflow: ellipsis;
}

.block-input {
	margin-left: -1em;
	text-align: left;
	background: var(--blue-200) !important;
}

.block-output {
	margin-right: -1em;
	text-align: center;
	background: var(--green-200) !important;
	border-radius: 10% !important;
}
</style>