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
	<div class="ui-block-body">
		<Handle id={inputKey} type="target" position={Position.Left} class="handle-dot handle-input" />
		<Input
			value={String(block.inputs.in.value ?? '')}
			oninput={onInputChange}
			class="h-7 w-28 text-xs"
		/>
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
