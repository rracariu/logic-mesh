<script lang="ts">
	import { onMount } from 'svelte';
	import { Play, Pause, Copy, ClipboardPaste, RotateCcw, FilePlus, Blocks, Search } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectTrigger
	} from '$lib/components/ui/select';
	import { Separator } from '$lib/components/ui/separator';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { examplePrograms } from '$lib/Examples';
	import { useEngine } from '$lib/Engine';
	import type { BlockDesc, Program } from 'logic-mesh';

	interface Props {
		blocks: BlockDesc[];
		onAddBlock: (block: BlockDesc) => void;
		onReset: () => void;
		onCopy: () => void;
		onPaste: () => void;
		onLoad: (program: Program) => void;
	}

	let { blocks, onAddBlock, onReset, onCopy, onPaste, onLoad }: Props = $props();

	const { command } = useEngine();

	let isRunning = $state(true);
	let selectedIndex = $state<string>('1');
	let blockSearch = $state('');

	const curProgram = $derived(examplePrograms[Number(selectedIndex)]);

	const blocksFiltered = $derived(
		blocks.filter((b) => b.dis.toLowerCase().includes(blockSearch.toLowerCase()))
	);

	const categories = $derived(
		[...new Set(blocksFiltered.map((b) => b.category.toLowerCase()))]
	);

	function blocksForCategory(category: string) {
		return blocksFiltered.filter((b) => b.category.toLowerCase() === category);
	}

	function capitalize(s: string) {
		return s.charAt(0).toUpperCase() + s.slice(1);
	}

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

<div class="flex items-center gap-2 rounded-lg border bg-background px-3 py-2 shadow-md">
	<!-- Block selector dropdown -->
	<DropdownMenu.Root onOpenChange={(open) => { if (!open) blockSearch = ''; }}>
		<DropdownMenu.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" class="gap-2">
					<Blocks class="h-4 w-4" />
					Add Block
				</Button>
			{/snippet}
		</DropdownMenu.Trigger>
		<DropdownMenu.Content class="w-48" align="start">
			<div class="flex items-center gap-2 px-2 pb-1.5">
				<Search class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
				<input
					type="text"
					placeholder="Search..."
					class="h-7 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
					bind:value={blockSearch}
					onkeydown={(e) => e.stopPropagation()}
				/>
			</div>
			<DropdownMenu.Separator />
			{#if blockSearch.length > 0}
				{#each blocksFiltered as block (block.lib + '/' + block.name)}
					<DropdownMenu.Item onclick={() => onAddBlock(block)}>
						<span>{capitalize(block.dis)}</span>
						<span class="ml-auto text-xs text-muted-foreground">{capitalize(block.category)}</span>
					</DropdownMenu.Item>
				{/each}
				{#if blocksFiltered.length === 0}
					<div class="px-2 py-1.5 text-sm text-muted-foreground">No results</div>
				{/if}
			{:else}
				{#each categories as category, i}
					{#if i > 0}
						<DropdownMenu.Separator />
					{/if}
					<DropdownMenu.Sub>
						<DropdownMenu.SubTrigger>{capitalize(category)}</DropdownMenu.SubTrigger>
						<DropdownMenu.SubContent>
							{#each blocksForCategory(category) as block (block.name)}
								<DropdownMenu.Item onclick={() => onAddBlock(block)}>
									{capitalize(block.dis)}
								</DropdownMenu.Item>
							{/each}
						</DropdownMenu.SubContent>
					</DropdownMenu.Sub>
				{/each}
			{/if}
		</DropdownMenu.Content>
	</DropdownMenu.Root>

	<Separator orientation="vertical" class="h-6" />

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
