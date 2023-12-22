<script setup lang="ts">
import Checkbox from 'primevue/checkbox';
import { Handle, Position, } from '@vue-flow/core';
import { capitalize } from 'vue';
import { Block } from '../../lib/Block';
import { useEngine } from '../../lib/Engine';
import BlockCommons from '../BlockCommons.vue';

const props = defineProps<{ data: Block }>()
const { command } = useEngine()

function onInputChange(data: boolean) {
	command.writeBlockOutput(props.data.id, Object.keys(props.data.outputs)[0] ?? '', data)
}

</script>

<template>
	<BlockCommons :data="data">
		<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left" class="handle-input" />

		<div class="flex align-items-start justify-content-center m-1 border-round p-1">
			{{ capitalize(Object.keys(data.inputs)[0] ?? '') }}
		</div>

		<div class="flex flex-grow-1  align-items-center justify-content-center m-1 border-round">
			<Checkbox v-model="data.inputs.in.value" :binary="true" v-on:update:model-value="onInputChange" />
		</div>

		<div class="flex align-items-end justify-content-center m-1 p-1 border-round">
			{{ capitalize(Object.keys(data.outputs)[0] ?? '') }}
		</div>

		<Handle :id="Object.keys(data.outputs)[0] ?? 'out'" type="source" :position="Position.Right"
			class="handle-output" />

	</BlockCommons>
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