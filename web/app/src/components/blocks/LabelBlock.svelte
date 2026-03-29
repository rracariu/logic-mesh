<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);
	const inputKey = $derived(Object.keys(block.inputs)[0] ?? 'in');

	const displayValue = $derived.by(() => {
		const val = block.inputs.in.value;
		if (val == null) return '-';
		if (typeof val === 'number') return Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(val);
		if (typeof val === 'boolean') return val ? 'true' : 'false';
		if (typeof val === 'object') return JSON.stringify(val);
		return String(val);
	});
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<Handle id={inputKey} type="target" position={Position.Left} class="handle-dot handle-input" />
		<span class="label-text">{displayValue}</span>
	</div>
</BlockCommons>

<style>
	.ui-block-body {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		position: relative;
	}

	.label-text {
		font-size: 13px;
		font-weight: 500;
		min-width: 40px;
		text-align: center;
		user-select: text;
	}

	:global(.handle-dot) {
		width: 8px !important;
		height: 8px !important;
		border-radius: 50% !important;
		min-width: 0 !important;
		border: 1.5px solid white !important;
	}
	:global(.handle-input) { background: #6b9eff !important; }
</style>
