<script lang="ts">
	import { onMount } from 'svelte';
	import { Play, Pause, Copy, ClipboardPaste, RotateCcw, FilePlus } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectTrigger
	} from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import { examplePrograms } from '$lib/Examples';
	import { useEngine } from '$lib/Engine';
	import type { Program } from 'logic-mesh';

	interface Props {
		onReset: () => void;
		onCopy: () => void;
		onPaste: () => void;
		onLoad: (program: Program) => void;
	}

	let { onReset, onCopy, onPaste, onLoad }: Props = $props();

	const { command } = useEngine();

	let isRunning = $state(true);
	let selectedIndex = $state<string>('1');

	const curProgram = $derived(examplePrograms[Number(selectedIndex)]);

	onMount(() => {
		onLoad(curProgram);
	});

	function onPauseResume() {
		if (isRunning) {
			command.pauseExecution();
		} else {
			command.resumeExecution();
		}
		isRunning = !isRunning;
	}

	function handleNew() {
		isRunning = true;
		selectedIndex = '';
		onReset();
	}

	function handleReset() {
		isRunning = true;
		onReset();
	}

	function handlePaste() {
		if (!isRunning) {
			command.resumeExecution().then(() => {
				isRunning = true;
				onPaste();
			});
		} else {
			onPaste();
		}
	}
</script>

<div class="flex min-w-[30em] items-center gap-2 rounded-lg border bg-background px-3 py-2 shadow-md">
	<!-- Program selector -->
	<Select
		type="single"
		value={selectedIndex}
		onValueChange={(v) => {
			selectedIndex = v;
			onLoad(examplePrograms[Number(v)]);
		}}
	>
		<SelectTrigger class="w-40">
			{curProgram?.name ?? 'Select a Program'}
		</SelectTrigger>
		<SelectContent>
			{#each examplePrograms as program, i}
				<SelectItem value={String(i)}>{program.name}</SelectItem>
			{/each}
		</SelectContent>
	</Select>

	<Separator orientation="vertical" class="h-6" />

	<!-- Play/Pause -->
	<Button variant="outline" size="icon" title="Pause/Resume execution" onclick={onPauseResume}>
		{#if isRunning}
			<Pause class="h-4 w-4" />
		{:else}
			<Play class="h-4 w-4" />
		{/if}
	</Button>

	<Separator orientation="vertical" class="h-6" />

	<!-- Actions -->
	<Button variant="outline" size="icon" title="New program" onclick={handleNew}>
		<FilePlus class="h-4 w-4" />
	</Button>
	<Button variant="outline" size="icon" title="Copy program" onclick={onCopy}>
		<Copy class="h-4 w-4" />
	</Button>
	<Button variant="outline" size="icon" title="Paste program" onclick={handlePaste}>
		<ClipboardPaste class="h-4 w-4" />
	</Button>
	<Button variant="outline" size="icon" title="Reset engine" onclick={handleReset}>
		<RotateCcw class="h-4 w-4" />
	</Button>
</div>
