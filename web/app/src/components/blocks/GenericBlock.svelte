<script lang="ts">
	import { Handle, Position, type IsValidConnection } from '@xyflow/svelte';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);

	const handlePos = (index: number) => `top: ${index * 1.5 + 3.0}em`;

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

	const blockStyle = $derived(
		`width: 100%; height: ${Object.keys(inputPins).length * 1.3 + 3.0}em;`
	);

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
	<div style={blockStyle}>
		{#each Object.entries(inputPins) as [name, input], index}
			<Handle
				id={input.name}
				isValidConnection={validConnection}
				type="target"
				position={Position.Left}
				style={handlePos(index)}
				class="block-input"
			>
				<span class="pin-label">{name}{input.value != null ? ` ${format(input.value)}` : ''}</span>
			</Handle>
		{/each}

		{#each Object.entries(block.outputs) as [name, output], index}
			<Handle
				id={output.name}
				isValidConnection={validConnection}
				type="source"
				position={Position.Right}
				style={handlePos(index)}
				class="block-output"
			>
				<span class="pin-label">{name}{output.value != null ? ` ${format(output.value)}` : ''}</span>
			</Handle>
		{/each}
	</div>
</BlockCommons>

<style>
	:global([class*='block-']) {
		font-size: x-small;
		padding: 1px;
		display: inline-table;
		border-radius: 10% !important;
		min-width: 5em !important;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	:global(.block-input) {
		margin-left: -1em;
		text-align: left;
		background: #93c5fd !important;
	}

	:global(.block-output) {
		margin-right: -1em;
		text-align: center;
		background: #86efac !important;
		border-radius: 10% !important;
	}

	.pin-label {
		font-size: x-small;
		pointer-events: none;
	}
</style>
