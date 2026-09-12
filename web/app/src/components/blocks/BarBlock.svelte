<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { onValue } from '$lib/UiConnector';
  import { useWidgetConfig } from '$lib/WidgetConfig.svelte';
  import { numericValue } from '$lib/utils';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);
  const widgetConfig = useWidgetConfig(() => block.widget);
  const config = $derived(widgetConfig.config);

  let raw = $state<unknown>(undefined);
  $effect(() => onValue(block.id, (v) => (raw = v)));

  const value = $derived(numericValue(raw) ?? 0);
  const min = $derived(numericValue(config.min) ?? 0);
  const max = $derived(numericValue(config.max) ?? 100);
  const label = $derived(String(config.label ?? ''));

  const pct = $derived.by(() => {
    const span = max - min;
    if (span === 0) return 0;
    return Math.max(0, Math.min(100, ((value - min) / span) * 100));
  });

  const display = $derived(
    Number.isFinite(value)
      ? Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value)
      : '-',
  );
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Handle
      id="in"
      type="target"
      position={Position.Left}
      class="handle-dot handle-input"
    />

    <div class="bar-area">
      <span class="bar-value">{display}</span>
      <div class="bar-track">
        <div class="bar-fill" style:height={`${pct}%`}></div>
      </div>
      {#if label}
        <span class="bar-label">{label}</span>
      {/if}
    </div>
  </div>
</BlockCommons>

<style>
  .ui-block-body {
    display: flex;
    align-items: stretch;
    gap: 10px;
    padding: 6px 10px;
    position: relative;
  }

  .bar-area {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-width: 50px;
  }

  .bar-value {
    font-size: 11px;
    font-weight: 600;
  }

  .bar-track {
    width: 14px;
    height: 64px;
    background: var(--muted);
    border: 1px solid var(--border);
    border-radius: 3px;
    display: flex;
    flex-direction: column-reverse;
    overflow: hidden;
  }

  .bar-fill {
    width: 100%;
    background: linear-gradient(180deg, #6bcf7f 0%, #3ecf6b 100%);
    transition: height 150ms ease;
  }

  .bar-label {
    font-size: 10px;
    opacity: 0.75;
    max-width: 100px;
    text-align: center;
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
