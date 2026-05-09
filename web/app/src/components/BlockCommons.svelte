<script lang="ts">
	import type { Snippet } from 'svelte';
	import { tick } from 'svelte';
	import { X } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { model } from '$lib/model.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: Block;
		children: Snippet;
	}

	let { data, children }: Props = $props();

	const isSelected = $derived(
		(model.currentBlock?.data as { value: Block } | undefined)?.value?.id === data.id
	);

	let editing = $state(false);
	let inputEl: HTMLInputElement | undefined = $state(undefined);

	async function startEdit() {
		editing = true;
		await tick();
		inputEl?.focus();
		inputEl?.select();
	}

	function commitEdit(e: Event) {
		const v = (e.target as HTMLInputElement).value.trim();
		data.label = v;
		editing = false;
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
		else if (e.key === 'Escape') {
			editing = false;
		}
	}

	function remove() {
		model.removeBlock(data.id);
	}
</script>

<div class="node-container" style={isSelected ? 'box-shadow: 0 0 0 2px var(--primary);' : ''}>
	<div class="node-header">
		<span class="node-title-row">
			<span class="node-title" title={data.desc.doc}>{data.desc.name}</span>
			{#if editing}
				<input
					bind:this={inputEl}
					class="node-label-input nodrag"
					type="text"
					value={data.label ?? ''}
					placeholder="add label"
					onblur={commitEdit}
					onkeydown={onKey}
				/>
			{:else}
				<button
					type="button"
					class="node-label nodrag"
					class:node-label-empty={!data.label}
					title="Click to edit label"
					onclick={startEdit}
				>
					{data.label ? data.label : 'add label'}
				</button>
			{/if}
		</span>
		<Button
			variant="ghost"
			size="icon"
			class="h-4 w-4 text-inherit opacity-50 hover:opacity-100"
			onclick={remove}
			aria-label="Remove block"
		>
			<X class="h-3 w-3" />
		</Button>
	</div>
	<div class="node-body">
		{@render children()}
	</div>
</div>

<style>
	.node-container {
		width: 100%;
		border-radius: 5px;
		overflow: hidden;
	}

	.node-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 2px 6px;
		background: var(--muted);
		border-bottom: 1px solid var(--border);
		gap: 6px;
		min-width: 0;
	}

	.node-title-row {
		display: flex;
		align-items: baseline;
		gap: 6px;
		min-width: 0;
		flex: 1;
	}

	.node-title {
		font-size: 11px;
		font-weight: 600;
		flex-shrink: 0;
	}

	.node-label {
		font-size: 11px;
		font-weight: 400;
		font-style: italic;
		opacity: 0.75;
		background: transparent;
		border: none;
		padding: 0;
		margin: 0;
		cursor: text;
		text-align: left;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
		max-width: 160px;
	}

	.node-label:hover {
		opacity: 1;
		text-decoration: underline dotted;
		text-underline-offset: 2px;
	}

	.node-label-empty {
		opacity: 0.4;
	}

	.node-label-input {
		font-size: 11px;
		font-style: italic;
		background: transparent;
		border: none;
		border-bottom: 1px solid var(--primary);
		padding: 0;
		margin: 0;
		outline: none;
		min-width: 0;
		max-width: 160px;
		flex: 1;
	}

	.node-title-row :global(.node-label),
	.node-title-row :global(.node-label-input) {
		font-family: inherit;
	}

	.node-body {
		padding: 4px 0;
	}
</style>
