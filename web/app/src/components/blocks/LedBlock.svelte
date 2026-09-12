<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { onValue } from '$lib/UiConnector';
  import { useWidgetConfig } from '$lib/WidgetConfig.svelte';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);
  const widgetConfig = useWidgetConfig(() => block.widget);
  const config = $derived(widgetConfig.config);

  let raw = $state<unknown>(undefined);
  $effect(() => onValue(block.id, (v) => (raw = v)));

  const on = $derived(Boolean(raw));
  const label = $derived(String(config.label ?? ''));
  const color = $derived(String(config.color ?? '#3ecf6b'));
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Handle
      id="in"
      type="target"
      position={Position.Left}
      class="handle-dot handle-input"
    />

    <div class="led-area">
      <span
        class="led"
        style:background={on ? color : 'transparent'}
        style:border-color={color}
      >
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
