<script setup lang="ts">
import Checkbox from 'primevue/checkbox';
import { Handle, Position, } from '@vue-flow/core';
import { capitalize } from 'vue';
import { Block } from '../../lib/Block';
import { useEngine } from '../../lib/Engine';

const props = defineProps<{ data: Block }>()
const { command } = useEngine()

function onInputChange(data: boolean) {
	command.writeBlockOutput(props.data.id, Object.keys(props.data.outputs)[0] ?? '', data)
}

</script>

<template>
	<div class="flex">
		<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left" class="handle-input" />

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.inputs)[0] ?? '') }}
		</div>

		<div class="flex align-items-center justify-content-center m-1 border-round">
			<Checkbox v-model="data.inputs.in.value" :binary="true" v-on:update:model-value="onInputChange" />
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