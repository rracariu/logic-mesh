<script lang="ts">
	import type { Block } from '$lib/Block';
	import GenericBlock from './blocks/GenericBlock.svelte';
	import InputBlock from './blocks/InputBlock.svelte';
	import CheckboxBlock from './blocks/CheckboxBlock.svelte';
	import ChartBlock from './blocks/ChartBlock.svelte';
	import GaugeBlock from './blocks/GaugeBlock.svelte';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);
</script>

{#if block.desc.implementation === 'native'}
	<GenericBlock {data} />
{:else if block.desc.name === 'Input'}
	<InputBlock {data} />
{:else if block.desc.name === 'Checkbox'}
	<CheckboxBlock {data} />
{:else if block.desc.name === 'Chart'}
	<ChartBlock {data} />
{:else if block.desc.name === 'Gauge'}
	<GaugeBlock {data} />
{:else}
	<GenericBlock {data} />
{/if}
