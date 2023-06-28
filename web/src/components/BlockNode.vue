<script setup lang="ts">
import { Connection, Handle, Position, } from '@vue-flow/core';

import { computed } from 'vue';
import { Block } from '../lib/Block';
import { currentBlock } from '../lib/Model'

const props = defineProps<{ data: Block }>()

const handlePos = (index: number) => `top: ${index + index / 2 + 3.5}em`

const blockStyle = computed(() => {
	let css = `width: 100%; height: ${Object.keys(props.data.inputs).length + 4.5}em; `

	if (currentBlock.value?.data.id === props.data.id) {
		css += 'box-shadow: 2px 2px 7px 3px var(--surface-200);'
	}

	return css
})

const validConnection = (conn: Connection) => {
	return conn.source !== conn.target
}

const format = (value: unknown) => {
	return typeof value === 'number' ? Intl.NumberFormat().format(value) : value
}
</script>

<template>
	<div :style="blockStyle">
		<div class="header">
			{{ data.desc.name }}
		</div>
		<Handle v-for="(input, name, index) in data.inputs" :key="name" :id="input.name"
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