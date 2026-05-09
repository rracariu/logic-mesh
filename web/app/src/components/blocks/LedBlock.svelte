<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);

	const on = $derived(Boolean(block.inputs.in?.value));
	const label = $derived(String(block.inputs.label?.value ?? ''));
	const color = $derived(String(block.inputs.color?.value ?? '#3ecf6b'));
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<div class="pin-stack">
			<div class="pin-row">
				<Handle id="in" type="target" position={Position.Left} class="handle-dot handle-input" />
				<span class="pin-name">in</span>
			</div>
			<div class="pin-row">
				<Handle id="label" type="target" position={Position.Left} class="handle-dot handle-input" />
				<span class="pin-name">label</span>
			</div>
			<div class="pin-row">
				<Handle id="color" type="target" position={Position.Left} class="handle-dot handle-input" />
				<span class="pin-name">color</span>
			</div>
		</div>

		<div class="led-area">
			<span class="led" style:background={on ? color : 'transparent'} style:border-color={color}>
			</span>
			{#if label}
				<span class="led-label">{label}</span>
			{/if}
		</div>
	</div>
</BlockCommons>

<style>
	.ui-block-body {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		position: relative;
	}

	.pin-stack {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.pin-row {
		display: flex;
		align-items: center;
		padding: 1px 8px;
		gap: 6px;
		min-height: 18px;
		position: relative;
	}

	.pin-name {
		font-size: 11px;
		opacity: 0.85;
	}

	.led-area {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		min-width: 60px;
	}

	.led {
		display: inline-block;
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: 2px solid;
		box-shadow: 0 0 6px rgba(0, 0, 0, 0.15) inset;
		transition: background 120ms ease;
	}

	.led-label {
		font-size: 10px;
		text-align: center;
		max-width: 90px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	:global(.handle-dot) {
		width: 8px !important;
		height: 8px !important;
		border-radius: 50% !important;
		min-width: 0 !important;
		border: 1.5px solid white !important;
	}
	:global(.handle-input) {
		background: #6b9eff !important;
	}
</style>
