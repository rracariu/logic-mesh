<script lang="ts">
	import { Handle, Position, type IsValidConnection } from '@xyflow/svelte';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);

	const inputPins = $derived.by(() => {
		const ins = block.inputs;
		const keys = Object.keys(ins);

		// If all keys match pattern like "in1", "in2" etc., trim trailing unconnected
		if (!keys.every((k) => k.match(/^[a-zA-Z]+[0-9]+$/))) {
			return ins;
		}

		const entries = Object.entries(ins);
		let lastConnected = 0;
		for (let i = 0; i < entries.length; i++) {
			if (entries[i][1].isConnected) {
				lastConnected = i > 0 ? i + 1 : i;
			}
		}

		const res: Block['inputs'] = {};
		for (let i = 0; i < Math.min(lastConnected + 2, entries.length); i++) {
			res[entries[i][0]] = entries[i][1];
		}
		return res;
	});

	const validConnection: IsValidConnection = (conn) => conn.source !== conn.target;

	function format(value: unknown): string {
		if (typeof value === 'number') return Intl.NumberFormat().format(value);
		if (typeof value === 'string') return value.slice(0, 5);
		if (typeof value === 'boolean') return value ? 'true' : 'false';
		if (Array.isArray(value)) return '[]';
		if (typeof value === 'object') return '{}';
		return '-';
	}
</script>

<BlockCommons data={block}>
	<!-- Outputs (right-aligned) -->
	{#each Object.entries(block.outputs) as [name, output]}
		<div class="pin-row pin-row-output">
			<span class="pin-name">{name}</span>
			<span class="pin-value">{output.value != null ? format(output.value) : ''}</span>
			<Handle
				id={output.name}
				isValidConnection={validConnection}
				type="source"
				position={Position.Right}
				class="handle-dot handle-output"
			/>
		</div>
	{/each}

	<!-- Separator if both inputs and outputs exist -->
	{#if Object.keys(block.outputs).length > 0 && Object.keys(inputPins).length > 0}
		<div class="pin-separator"></div>
	{/if}

	<!-- Inputs (left-aligned) -->
	{#each Object.entries(inputPins) as [name, input]}
		<div class="pin-row pin-row-input">
			<Handle
				id={input.name}
				isValidConnection={validConnection}
				type="target"
				position={Position.Left}
				class="handle-dot handle-input"
			/>
			<span class="pin-name">{name}</span>
			<span class="pin-value">{input.value != null ? format(input.value) : ''}</span>
		</div>
	{/each}
</BlockCommons>

<style>
	.pin-row {
		display: flex;
		align-items: center;
		padding: 1px 8px;
		gap: 6px;
		min-height: 20px;
		position: relative;
	}

	.pin-row-output {
		justify-content: flex-end;
	}

	.pin-row-input {
		justify-content: flex-start;
	}

	.pin-name {
		font-size: 11px;
	}

	.pin-value {
		font-size: 10px;
		opacity: 0.6;
	}

	.pin-separator {
		border-top: 1px solid var(--border);
		margin: 2px 8px;
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

	:global(.handle-output) {
		background: #6bcf7f !important;
	}
</style>
