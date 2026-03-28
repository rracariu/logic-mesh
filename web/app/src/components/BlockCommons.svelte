<script lang="ts">
	import type { Snippet } from 'svelte';
	import { X } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import { model } from '$lib/model.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: Block;
		children: Snippet;
	}

	let { data, children }: Props = $props();

	const isSelected = $derived((model.currentBlock?.data as { value: Block } | undefined)?.value?.id === data.id);

	function remove() {
		model.removeBlock(data.id);
	}
</script>

<div
	class="node-container"
	style={isSelected ? 'box-shadow: 0 0 0 2px var(--primary);' : ''}
>
	<div class="node-header">
		<span class="node-title" title={data.desc.doc}>{data.desc.name}</span>
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
	}

	.node-title {
		font-size: 11px;
		font-weight: 600;
	}

	.node-body {
		padding: 4px 0;
	}
</style>
