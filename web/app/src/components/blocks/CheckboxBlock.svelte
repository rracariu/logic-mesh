<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { onMount } from 'svelte';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { pushValue } from '$lib/UiConnector';
  import { useValueFeedback, useWidgetConfig } from '$lib/WidgetConfig.svelte';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);
  const widgetConfig = useWidgetConfig(() => block.widget);
  const config = $derived(widgetConfig.config);
  const checked = $derived(Boolean(config.value ?? false));

  // A checkbox click is instantaneous, so feedback always applies; it
  // updates the displayed state without being re-pushed to the engine.
  useValueFeedback(
    () => block.widget,
    (value) => {
      if (value == null || !block.widget) return;
      block.widget.config = { ...block.widget.config, value: Boolean(value) };
    },
  );

  onMount(() => {
    pushValue(block.id, checked);
  });

  function onCheckedChange(next: boolean) {
    if (block.widget) {
      block.widget.config = { ...block.widget.config, value: next };
    }
    pushValue(block.id, next);
  }
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Checkbox {checked} {onCheckedChange} />
    <Handle
      id="out"
      type="source"
      position={Position.Right}
      class="handle-dot handle-output"
    />
  </div>
</BlockCommons>

<style>
  .ui-block-body {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6px 10px;
    position: relative;
  }

  :global(.handle-dot) {
    width: 8px !important;
    height: 8px !important;
    border-radius: 50% !important;
    min-width: 0 !important;
    border: 1.5px solid white !important;
  }
  :global(.handle-output) {
    background: #6bcf7f !important;
  }
</style>
