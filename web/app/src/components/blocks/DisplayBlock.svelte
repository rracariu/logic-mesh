<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { onValue } from '$lib/UiConnector';
  import { useWidgetConfig } from '$lib/WidgetConfig.svelte';
  import { isHaystackNumber, numericValue, unitOf } from '$lib/utils';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);
  const widgetConfig = useWidgetConfig(() => block.widget);
  const config = $derived(widgetConfig.config);

  let raw = $state<unknown>(undefined);
  $effect(() => onValue(block.id, (v) => (raw = v)));

  // Prefer the unit on the upstream Number; fall back to the configured unit.
  const unit = $derived(unitOf(raw) ?? String(config.unit ?? ''));
  const label = $derived(String(config.label ?? ''));

  const display = $derived.by(() => {
    if (raw == null) return '—';
    if (typeof raw === 'boolean') return raw ? 'true' : 'false';
    if (typeof raw === 'number' || isHaystackNumber(raw)) {
      const n = numericValue(raw);
      return n == null
        ? '—'
        : Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(n);
    }
    return String(raw);
  });
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Handle
      id="in"
      type="target"
      position={Position.Left}
      class="handle-dot handle-input"
    />

    <div class="display-area">
      {#if label}
        <span class="display-label">{label}</span>
      {/if}
      <span class="display-value">
        {display}{#if unit}<span class="display-unit"> {unit}</span>{/if}
      </span>
    </div>
  </div>
</BlockCommons>

<style>
  .ui-block-body {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    position: relative;
  }

  .display-area {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    min-width: 80px;
  }

  .display-label {
    font-size: 9px;
    opacity: 0.6;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .display-value {
    font-size: 16px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .display-unit {
    font-size: 11px;
    font-weight: 500;
    opacity: 0.65;
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
