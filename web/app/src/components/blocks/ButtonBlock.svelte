<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { Button } from '$lib/components/ui/button';
	import BlockCommons from '../BlockCommons.svelte';
	import { useEngine } from '$lib/Engine';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();
	const { command } = useEngine();

	const block = $derived(data.value);
	const outputKey = $derived(Object.keys(block.outputs)[0] ?? 'out');

	function onPress() {
		block.outputs.out.value = true;
		command.writeBlockOutput(block.id, outputKey, true);
	}

	function onRelease() {
		block.outputs.out.value = false;
		command.writeBlockOutput(block.id, outputKey, false);
	}
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<Button
			variant="outline"
			size="sm"
			class="h-7 text-xs"
			onpointerdown={onPress}
			onpointerup={onRelease}
			onpointerleave={onRelease}
		>
			{block.outputs.out.value ? 'ON' : 'OFF'}
		</Button>
		<Handle id={outputKey} type="source" position={Position.Right} class="handle-dot handle-output" />
	</div>
</BlockCommons>

<style>
	.ui-block-body {
		display: flex;
		align-items: center;
		justify-content: center;
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
	:global(.handle-output) { background: #6bcf7f !important; }
</style>
