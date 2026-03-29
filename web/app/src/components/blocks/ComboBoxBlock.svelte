<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import * as Select from '$lib/components/ui/select';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { Plus } from 'lucide-svelte';
	import BlockCommons from '../BlockCommons.svelte';
	import { useEngine } from '$lib/Engine';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();
	const { command } = useEngine();

	const block = $derived(data.value);
	const inputKey = $derived(Object.keys(block.inputs)[0] ?? 'in');
	const outputKey = $derived(Object.keys(block.outputs)[0] ?? 'out');

	let customItems: string[] = $state([]);
	let customEntry = $state('');
	let showCustomInput = $state(false);

	// Items from the CSV input + any custom-added items
	const items = $derived.by(() => {
		const csv = String(block.inputs.in.value ?? '');
		const fromInput = csv
			.split(',')
			.map((s) => s.trim())
			.filter(Boolean);
		// Merge, deduplicate, keep order
		const all = [...fromInput];
		for (const c of customItems) {
			if (!all.includes(c)) all.push(c);
		}
		return all;
	});

	const selected = $derived(
		block.outputs.out.value != null ? String(block.outputs.out.value) : undefined
	);

	function onSelect(value: string | undefined) {
		if (value != null) {
			block.outputs.out.value = value;
			command.writeBlockOutput(block.id, outputKey, value);
		}
	}

	function addCustomItem() {
		const val = customEntry.trim();
		if (val && !items.includes(val)) {
			customItems = [...customItems, val];
		}
		if (val) {
			onSelect(val);
		}
		customEntry = '';
		showCustomInput = false;
	}

	function onCustomKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			addCustomItem();
		} else if (e.key === 'Escape') {
			showCustomInput = false;
			customEntry = '';
		}
	}
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<Handle id={inputKey} type="target" position={Position.Left} class="handle-dot handle-input" />

		<div class="combo-container">
			<Select.Root type="single" value={selected} onValueChange={onSelect}>
				<Select.Trigger class="h-7 w-32 text-xs">
					{selected ?? 'Select...'}
				</Select.Trigger>
				<Select.Content>
					{#each items as item}
						<Select.Item value={item} label={item} />
					{/each}
				</Select.Content>
			</Select.Root>

			{#if showCustomInput}
				<div class="custom-entry">
					<Input
						bind:value={customEntry}
						onkeydown={onCustomKeydown}
						placeholder="New item..."
						class="h-6 w-24 text-xs"
					/>
					<Button variant="ghost" size="icon" class="h-6 w-6" onclick={addCustomItem}>
						<Plus class="h-3 w-3" />
					</Button>
				</div>
			{:else}
				<Button
					variant="ghost"
					size="icon"
					class="h-6 w-6"
					onclick={() => (showCustomInput = true)}
					aria-label="Add custom item"
				>
					<Plus class="h-3 w-3" />
				</Button>
			{/if}
		</div>

		<Handle id={outputKey} type="source" position={Position.Right} class="handle-dot handle-output" />
	</div>
</BlockCommons>

<style>
	.ui-block-body {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		position: relative;
	}

	.combo-container {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.custom-entry {
		display: flex;
		align-items: center;
		gap: 2px;
	}

	:global(.handle-dot) {
		width: 8px !important;
		height: 8px !important;
		border-radius: 50% !important;
		min-width: 0 !important;
		border: 1.5px solid white !important;
	}
	:global(.handle-input) { background: #6b9eff !important; }
	:global(.handle-output) { background: #6bcf7f !important; }
</style>
