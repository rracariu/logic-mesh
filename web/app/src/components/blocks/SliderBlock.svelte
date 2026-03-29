<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
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

	const numValue = $derived(Number(block.inputs.in.value ?? 0));

	const min = $derived(Number(block.inputs.min?.value ?? 0));
	const max = $derived(Number(block.inputs.max?.value ?? 100));
	const step = $derived(Number(block.inputs.step?.value ?? 1));

	function onSliderInput(event: Event) {
		const val = Number((event.target as HTMLInputElement).value);
		block.outputs.out.value = val;
		command.writeBlockOutput(block.id, outputKey, val);
	}
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<Handle id={inputKey} type="target" position={Position.Left} class="handle-dot handle-input" />

		<div class="slider-container">
			<input
				type="range"
				min={min}
				max={max}
				step={step}
				value={numValue}
				oninput={onSliderInput}
				class="slider"
			/>
			<span class="slider-value">{numValue}</span>
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

	.slider-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
	}

	.slider {
		width: 100px;
		height: 4px;
		accent-color: var(--primary);
		cursor: pointer;
	}

	.slider-value {
		font-size: 10px;
		opacity: 0.7;
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
