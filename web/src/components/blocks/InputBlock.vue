<script setup lang="ts">
import { Handle, Position, } from '@vue-flow/core';
import InputTex from 'primevue/inputtext';
import { capitalize, onMounted } from 'vue';
import { Block } from '../../lib/Block';
import { useEngine } from '../../lib/Engine';
import BlockCommons from '../BlockCommons.vue';

const props = defineProps<{ data: Block }>()
const { command } = useEngine()

onMounted(() => {
	if (props.data.inputs.in.value == null && props.data.outputs.out.value != null) {
		props.data.inputs.in.value = props.data.outputs.out.value
	}
})

function onInputChange(data: string) {
	props.data.outputs.out.value = data
	command.writeBlockOutput(props.data.id, Object.keys(props.data.outputs)[0] ?? '', data)
}

</script>

<template>
	<BlockCommons :data="data">
		<template #body>
			<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left"
				class="handle-input" />

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
		</template>
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