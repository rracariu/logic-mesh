<script setup lang="ts">
import InputTex from 'primevue/inputtext';
import { Handle, Position, } from '@vue-flow/core';
import { capitalize } from 'vue';

import { command } from '../../lib/Engine';
import { Block } from '../../lib/Block';

const props = defineProps<{ data: Block }>()

function onInputChange(data: string) {
	command.writeBlockOutput(props.data.id, Object.keys(props.data.outputs)[0] ?? '', data).then((val) => {
		console.log(val);
	})
}

</script>

<template>
	<div class="flex">
		<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left" class="handle-input" />

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.inputs)[0] ?? '') }}
		</div>

		<div class="flex align-items-center justify-content-center m-1 border-round">
			<InputTex :value="data.inputs.in.value" v-on:update:model-value="onInputChange" size="small" />
		</div>

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.outputs)[0] ?? '') }}
		</div>

		<Handle :id="Object.keys(data.outputs)[0] ?? 'out'" type="source" :position="Position.Right"
			class="handle-output" />
	</div>
</template>

<style scoped>
[class*="handle-"] {
	padding: 1px;
	display: inline-table;
	height: 1em;
	border-radius: 10%;
	overflow: hidden;
}

.handle-input {
	background: var(--blue-200);
}

.handle-output {
	background: var(--green-200);
}
</style>