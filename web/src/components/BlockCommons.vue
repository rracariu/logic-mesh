<script setup lang="ts">
import { computed } from 'vue';
import { Block } from '../lib/Block';
import { currentBlock } from '../lib/Model';
import Button from 'primevue/button';

const props = defineProps<{ data: Block }>()

const mainBlockClass = computed(() =>
	currentBlock.value?.data.id === props.data.id ? 'currentBlock' : ''
)

function remove() {
	if (currentBlock.value?.data.id === props.data.id) {
		dispatchEvent(new KeyboardEvent("keydown", { key: "Delete" }))
	}
}

</script>

<template>
	<div :class="mainBlockClass">
		<div class="flex justify-content-start header">
			<div class="flex flex-grow-1 align-items-center justify-content-center">
				<label :title="data.desc.doc"> {{ data.desc.name }} </label>
			</div>
			<div class="flex align-items-center justify-content-end">
				<Button aria-label="Remove" icon="pi pi-times" size="small" severity="info" style="height: 3px; width: 3px;"
					text rounded @click="remove" />
			</div>
		</div>
		<div class="flex">
			<slot name="body"></slot>
		</div>
	</div>
</template>

<style>
@import '../assets/js-block.css';

.currentBlock {
	box-shadow: 2px 2px 7px 3px var(--surface-200);
}
</style>