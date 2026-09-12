<script lang="ts">
  import type { Block } from '$lib/Block';
  import GenericBlock from './blocks/GenericBlock.svelte';
  import InputBlock from './blocks/InputBlock.svelte';
  import CheckboxBlock from './blocks/CheckboxBlock.svelte';
  import ChartBlock from './blocks/ChartBlock.svelte';
  import GaugeBlock from './blocks/GaugeBlock.svelte';
  import ButtonBlock from './blocks/ButtonBlock.svelte';
  import ComboBoxBlock from './blocks/ComboBoxBlock.svelte';
  import TableBlock from './blocks/TableBlock.svelte';
  import LabelBlock from './blocks/LabelBlock.svelte';
  import SliderBlock from './blocks/SliderBlock.svelte';
  import LedBlock from './blocks/LedBlock.svelte';
  import BarBlock from './blocks/BarBlock.svelte';
  import DisplayBlock from './blocks/DisplayBlock.svelte';
  import MultiChartBlock from './blocks/MultiChartBlock.svelte';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const kind = $derived(data.value.widget?.kind);
</script>

{#if kind === 'Input'}
  <InputBlock {data} />
{:else if kind === 'Checkbox'}
  <CheckboxBlock {data} />
{:else if kind === 'Chart'}
  <ChartBlock {data} />
{:else if kind === 'Gauge'}
  <GaugeBlock {data} />
{:else if kind === 'Button'}
  <ButtonBlock {data} />
{:else if kind === 'ComboBox'}
  <ComboBoxBlock {data} />
{:else if kind === 'Table'}
  <TableBlock {data} />
{:else if kind === 'Label'}
  <LabelBlock {data} />
{:else if kind === 'Slider'}
  <SliderBlock {data} />
{:else if kind === 'Led'}
  <LedBlock {data} />
{:else if kind === 'Bar'}
  <BarBlock {data} />
{:else if kind === 'Display'}
  <DisplayBlock {data} />
{:else if kind === 'MultiChart'}
  <MultiChartBlock {data} />
{:else}
  <GenericBlock {data} />
{/if}
