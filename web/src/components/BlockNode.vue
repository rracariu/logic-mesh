<script setup lang="ts">
import { Handle, Position } from '@vue-flow/core'
import { computed } from 'vue'


const props = defineProps({
	data: {
		type: Object,
		required: true,
	},
})


</script>

<template>
	<div style="height: 5em; padding-top: 4em;">
		{{ props.data.name }}

		<Handle id="a" type="target" :position="Position.Left" style="top: 1em;" class="blockInput">a</Handle>

		<Handle id="b" type="target" :position="Position.Left" style="top: 2.8em;" class="blockInput">b</Handle>

		<Handle id="c" type="source" :position="Position.Right" class="blockOutput">out</Handle>
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