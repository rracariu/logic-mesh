<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { Plus, Trash2 } from 'lucide-svelte';
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

	// Rows of key-value pairs
	let rows: { key: string; value: string }[] = $state([{ key: '', value: '' }]);

	function emitOutput() {
		const dict: Record<string, string> = {};
		for (const row of rows) {
			const k = row.key.trim();
			if (k) {
				dict[k] = row.value;
			}
		}
		block.outputs.out.value = dict;
		command.writeBlockOutput(block.id, outputKey, dict);
	}

	function addRow() {
		rows = [...rows, { key: '', value: '' }];
	}

	function removeRow(index: number) {
		rows = rows.filter((_, i) => i !== index);
		if (rows.length === 0) {
			rows = [{ key: '', value: '' }];
		}
		emitOutput();
	}

	function onKeyChange(index: number, event: Event) {
		rows[index].key = (event.target as HTMLInputElement).value;
		emitOutput();
	}

	function onValueChange(index: number, event: Event) {
		rows[index].value = (event.target as HTMLInputElement).value;
		emitOutput();
	}
</script>

<BlockCommons data={block}>
	<div class="ui-block-body">
		<div class="table-container">
			<div class="table-header">
				<span class="col-header">Key</span>
				<span class="col-header">Value</span>
				<span class="col-action"></span>
			</div>

			{#each rows as row, i}
				<div class="table-row">
					<Input
						value={row.key}
						oninput={(e) => onKeyChange(i, e)}
						placeholder="key"
						class="h-6 w-20 text-xs"
					/>
					<Input
						value={row.value}
						oninput={(e) => onValueChange(i, e)}
						placeholder="value"
						class="h-6 w-20 text-xs"
					/>
					<Button
						variant="ghost"
						size="icon"
						class="h-5 w-5"
						onclick={() => removeRow(i)}
						aria-label="Remove row"
					>
						<Trash2 class="h-3 w-3" />
					</Button>
				</div>
			{/each}

			<Button
				variant="ghost"
				size="sm"
				class="h-6 w-full text-xs"
				onclick={addRow}
			>
				<Plus class="h-3 w-3 mr-1" />
				Add row
			</Button>
		</div>

		<Handle id={outputKey} type="source" position={Position.Right} class="handle-dot handle-output" />
	</div>
</BlockCommons>

<style>
	.ui-block-body {
		display: flex;
		align-items: flex-start;
		padding: 6px 10px;
		position: relative;
	}

	.table-container {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.table-header {
		display: flex;
		gap: 4px;
		padding: 0 2px;
	}

	.col-header {
		font-size: 10px;
		font-weight: 600;
		opacity: 0.6;
		width: 80px;
	}

	.col-action {
		width: 20px;
	}

	.table-row {
		display: flex;
		align-items: center;
		gap: 4px;
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
