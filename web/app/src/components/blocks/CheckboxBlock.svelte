<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { Checkbox } from '$lib/components/ui/checkbox';
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

	function onCheckedChange(checked: boolean) {
		command.writeBlockOutput(block.id, outputKey, checked);
	}
</script>

<BlockCommons data={block}>
	<Handle id={inputKey} type="target" position={Position.Left} class="handle-input" />

	<div class="flex flex-col items-center gap-1 p-2">
		<span class="text-xs capitalize">{inputKey}</span>
		<Checkbox
			checked={Boolean(block.inputs.in.value)}
			onCheckedChange={onCheckedChange}
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
