<script setup lang="ts">
import { Connection, Handle, Position, } from '@vue-flow/core';
import { computed } from 'vue';
import { Block } from '../lib/Block';

const props = defineProps<{ data: Block }>()

const handlePos = (index: number) => `top: ${index + index / 2 + 3.5}em`

const blockHeight = computed(() => {
	return `height: ${props.data.inputs.length + 2}em; `
})

const validConnection = (conn: Connection) => {
	return conn.source !== conn.target
}

</script>

<template>
	<div :style="blockHeight">
		{{ data.name }}

		<Handle v-for="(input, index) in data.inputs" :key="input.name" :id="input.name"
			:is-valid-connection="validConnection" type="target" :position="Position.Left" :style="handlePos(index)"
			class="block-input">
			{{ input.name }}
		</Handle>


		<Handle v-for="(output, index) in data.outputs" :key="output.name" :id="output.name"
			:is-valid-connection="validConnection" type="source" :position="Position.Right" class="block-output"
			:style="handlePos(index)">
			{{ output.name }}
		</Handle>
	</div>
</template>

<style scoped>
[class*="block-"] {
	font-size: x-small;
	padding: 1px;
	display: inline-table;
	border-radius: 10% !important;
	min-width: 5em !important;
}

.block-input {
	margin-left: -1em;
	text-align: left;
	background: var(--blue-200) !important;
}

.block-output {
	margin-right: -1em;
	text-align: right;
	background: var(--green-200) !important;
	border-radius: 10% !important;
}
</style>