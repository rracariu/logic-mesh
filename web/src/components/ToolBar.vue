<script setup lang="ts">
import { ref } from 'vue';

import Toolbar from 'primevue/toolbar';
import Button from 'primevue/button';
import Dropdown from 'primevue/dropdown';

import { command } from '../lib/Engine';

const emit = defineEmits<{
	(event: 'reset'): void
}>()

const isRunning = ref(true)

function onPauseResume() {
	if (isRunning.value) {
		command.pauseExecution()
	} else {
		command.resumeExecution()
	}

	isRunning.value = !isRunning.value
}

const onReset = () => {
	command.resetEngine()
	isRunning.value = true
	emit('reset')
}

</script>

<template>
	<Toolbar>
		<template #start>
			<Dropdown optionLabel="programName" placeholder="Select a Program" class="w-full md:w-14rem" disabled />
		</template>
		<template #center>
			<Button title="Pause/Resume execution" :icon="'pi ' + (isRunning ? 'pi-pause' : 'pi-play')" rounded
				@click="onPauseResume" />
		</template>
		<template #end>
			<div class="flex-grow">
				<Button title="Copy program" :icon="'pi pi-copy'" class="m-1" disabled />
				<Button title="Paste program" :icon="'pi pi-file'" class="m-1" disabled />
				<Button title="Reset engine" :icon="'pi pi-refresh'" rounded @click="onReset" class="m-1" />
			</div>
		</template>
	</Toolbar>
</template>