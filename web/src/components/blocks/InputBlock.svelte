<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { onMount } from 'svelte';
	import { Input } from '$lib/components/ui/input';
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

	onMount(() => {
		if (block.inputs.in.value == null && block.outputs.out.value != null) {
			block.inputs.in.value = block.outputs.out.value;
		}
	});

	function onInputChange(event: Event) {
		const val = (event.target as HTMLInputElement).value;
		block.outputs.out.value = val;
		command.writeBlockOutput(block.id, outputKey, val);
	}
</script>

<BlockCommons data={block}>
	<Handle id={inputKey} type="target" position={Position.Left} class="handle-input" />

	<div class="flex flex-col items-center gap-1 p-1">
		<span class="text-xs capitalize">{inputKey}</span>
		<Input
			value={String(block.inputs.in.value ?? '')}
			oninput={onInputChange}
			class="h-7 w-24 text-xs"
		/>
		<span class="text-xs capitalize">{outputKey}</span>
	</div>

	<Handle id={outputKey} type="source" position={Position.Right} class="handle-output" />
</BlockCommons>

<style>
	:global(.handle-input) {
		background: #93c5fd !important;
	}
	:global(.handle-output) {
		background: #86efac !important;
	}
</style>
