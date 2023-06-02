<script setup lang="ts">
import { Connection, Handle, Position, } from '@vue-flow/core';
import { computed } from 'vue';


defineEmits(['outClick'])

const props = defineProps({
	data: {
		type: Object,
		required: true,
	},
})

const handlePos = (index: number) => `top: ${index + index / 2 + 3.5}em`

const blockHeight = computed(() => {
	return `height: ${props.data.inputs.length + 2}em; `
})

const validConnection = (conn: Connection) => conn.source !== conn.target
</script>

<template>
	<div :style="blockHeight" @click="$emit('outClick', data.id)">
		{{ data.name }}

		<Handle v-for="(input, index) in data.inputs" :key="input.name" :id="input.name"
			:is-valid-connection="validConnection" type="target" :position="Position.Left" :style="handlePos(index)"
			class="blockInput">
			{{ input.name }}
		</Handle>


		<Handle v-for="(output, index) in data.outputs" :key="output.name" :id="output.name"
			:is-valid-connection="validConnection" type="source" :position="Position.Right" class="blockOutput"
			:style="handlePos(index)">
			{{ output.name }}
		</Handle>
	</div>
</template>

<style>
.blockInput {
	font-size: x-small;
	padding: 1px;
	margin-left: -1em;
	display: inline-table;
	text-align: left;
	background: #a9a5d6 !important;
	border-radius: 10% !important;
	min-width: 5em !important;
}

.blockOutput {
	font-size: x-small;
	margin-right: -1em;
	padding: 1px;
	display: inline-table;
	background: rgb(230, 217, 134) !important;
	border-radius: 10% !important;
}
</style>