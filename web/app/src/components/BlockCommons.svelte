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
	class="w-full rounded"
	style={isSelected ? 'box-shadow: 2px 2px 7px 3px var(--muted);' : ''}
>
	<div class="header flex w-full items-center justify-between border-b px-1 py-0.5">
		<div class="flex flex-1 items-center justify-center">
			<span class="text-xs font-medium" title={data.desc.doc}>{data.desc.name}</span>
		</div>
		<Button
			variant="ghost"
			size="icon"
			class="h-5 w-5"
			onclick={remove}
			aria-label="Remove block"
		>
			<X class="h-3 w-3" />
		</Button>
	</div>
	<div class="flex min-w-full">
		{@render children()}
	</div>
</div>
