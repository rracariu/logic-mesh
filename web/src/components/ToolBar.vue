<script setup lang="ts">
import { onMounted, ref } from 'vue';

import Toolbar from 'primevue/toolbar';
import Button from 'primevue/button';
import Dropdown from 'primevue/dropdown';

import { command } from '../lib/Engine';
import { examplePrograms } from '../lib/Examples';
import { Program } from 'logic-mesh';

const emit = defineEmits<{
	(event: 'reset'): void,
	(event: 'copy'): void
	(event: 'paste'): void
	(event: 'load', program: Program): void
}>()

const isRunning = ref(true)
const curProgram = ref({} as Program)

onMounted(() => {
	curProgram.value = examplePrograms[1]
	emit('load', curProgram.value)
})

function onPauseResume() {
	if (isRunning.value) {
		command.pauseExecution()
	} else {
		command.resumeExecution()
	}

	isRunning.value = !isRunning.value
}

function onReset() {
	isRunning.value = true
	emit('reset')
}

function onPaste() {
	if (!isRunning.value) {
		command.resumeExecution().then(() => {
			isRunning.value = true
			emit('paste')
		})
	} else {
		emit('paste')
	}
}

</script>

<template>
	<Toolbar>
		<template #start>
			<Dropdown v-model="curProgram" :options="examplePrograms" optionLabel="name" placeholder="Select a Program"
				@change="emit('load', curProgram)" class="w-full md:w-14rem" />
		</template>
		<template #center>
			<Button title="Pause/Resume execution" :icon="'pi ' + (isRunning ? 'pi-pause' : 'pi-play')" rounded
				@click="onPauseResume" />
		</template>
		<template #end>
			<div class="flex-grow">
				<Button title="Copy program" :icon="'pi pi-copy'" class="m-1" @click="emit('copy')" />
				<Button title="Paste program" :icon="'pi pi-file'" class="m-1" @click="onPaste" />
				<Button title="Reset engine" :icon="'pi pi-refresh'" rounded @click="onReset" class="m-1" />
			</div>
		</template>
	</Toolbar>
</template>