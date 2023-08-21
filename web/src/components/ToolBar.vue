<script setup lang="ts">
import { ref } from 'vue';

import Toolbar from 'primevue/toolbar';
import Button from 'primevue/button';

import { command } from '../lib/Engine';

const isRunning = ref(true)

function onPauseResume() {
	if (isRunning.value) {
		command.pauseExecution()
	} else {
		command.resumeExecution()
	}

	isRunning.value = !isRunning.value
}


</script>

<template>
	<Toolbar>
		<template #center>
			<Button :icon="'pi ' + (isRunning ? 'pi-pause' : 'pi-play')" rounded @click="onPauseResume" />
		</template>
	</Toolbar>
</template>