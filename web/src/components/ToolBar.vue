<script setup lang="ts">
import { ref } from 'vue';

import Toolbar from 'primevue/toolbar';
import Button from 'primevue/button';

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
		<template #center>
			<Button title="Pause/Resume execution" :icon="'pi ' + (isRunning ? 'pi-pause' : 'pi-play')" rounded
				@click="onPauseResume" />
		</template>
		<template #end>
			<Button title="Reset engine" :icon="'pi pi-refresh'" rounded @click="onReset" />
		</template>
	</Toolbar>
</template>