<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import { computed } from 'vue'


const props = defineProps({
	data: {
		type: Object,
		required: true,
	},
})

const pos = (index: number) => `top: ${index + index / 2 + 1}em`

const blockHeight = computed(() => {
	return `height: ${props.data.inputs.length}em; padding-top: ${props.data.inputs.length - 1 + 0.3}em;`
})

</script>

<template>
	<div :style="blockHeight">
		{{ props.data.name }}

		<span v-for="(input, index) in props.data.inputs" :key="input.name">
			<Handle :id="input.name" type="target" :position="Position.Left" :style="pos(index)" class="blockInput">{{
				input.name
			}}
			</Handle>
		</span>

		<Handle :id="props.data.output.name" type="source" :position="Position.Right" class="blockOutput">{{
			props.data.output.name
		}}</Handle>
	</div>
</template>

<style>
.blockInput {
	font-size: x-small;
	padding: 1px;
	display: inline-table;
	background: burlywood !important;
	border-radius: 10% !important;
}

.blockOutput {
	font-size: x-small;
	margin-right: -14px;
	padding: 1px;
	display: inline-table;
	background: greenyellow !important;
	border-radius: 10% !important;
}
</style>