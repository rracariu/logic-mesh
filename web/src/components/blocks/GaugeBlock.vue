<script setup lang="ts">
import Button from 'primevue/button';
import Knob from 'primevue/knob';

import { Handle, Position, } from '@vue-flow/core';
import { capitalize } from 'vue';

import { Block } from '../../lib/Block';
import { command } from '../../lib/Engine';

const props = defineProps<{ data: Block }>()

function onChange(data: string) {
	props.data.outputs.out.value = data
	command.writeBlockOutput(props.data.id, Object.keys(props.data.outputs)[0] ?? '', data)
}

function increment() {
	let val = props.data.inputs.in.value as number ?? 0
	props.data.inputs.in.value = val + 1
}

function decrement() {
	let val = props.data.inputs.in.value as number ?? 0
	props.data.inputs.in.value = val - 1
}

</script>

<template>
	<div class="flex">
		<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left" class="handle-input" />

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.inputs)[0] ?? '') }}
		</div>

		<div class="grid align-items-center justify-content-center">
			<div class="col">
				<Knob v-model="data.inputs.in.value as number" v-on:update:model-value="onChange" readonly />
				<div class="grid align-items-center justify-content-center">
					<Button icon="pi pi-plus" text rounded aria-label="Increment" @click="increment" />
					<Button icon="pi pi-minus" text rounded aria-label="Decrement" @click="decrement" />
				</div>
			</div>
		</div>

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.outputs)[0] ?? '') }}
		</div>


		<Handle :id="Object.keys(data.outputs)[0] ?? 'out'" type="source" :position="Position.Right"
			class="handle-output" />

	</div>
</template>

<style scoped>
@import '../../assets/js-block.css';

.handle-input {
	background: var(--blue-200);
}

.handle-output {
	background: var(--green-200);
}
</style> 