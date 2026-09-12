<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { onMount } from 'svelte';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { pushValue } from '$lib/UiConnector';
  import { useValueFeedback, useWidgetConfig } from '$lib/WidgetConfig.svelte';
  import { numericValue } from '$lib/utils';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);
  const widgetConfig = useWidgetConfig(() => block.widget);
  const config = $derived(widgetConfig.config);

  const numValue = $derived(numericValue(config.value) ?? 0);
  const min = $derived(numericValue(config.min) ?? 0);
  const max = $derived(numericValue(config.max) ?? 100);
  const step = $derived(numericValue(config.step) ?? 1);

  // Feedback tracks the source while the user is not interacting; a
  // user edit wins during interaction and is never echoed back by
  // feedback. Dragging is tracked via pointer events besides focus —
  // WebKit/touch don't reliably focus a range input on drag.
  let dragging = $state(false);
  let focused = $state(false);
  const interacting = $derived(dragging || focused);
  const feedback = useValueFeedback(
    () => block.widget,
    (value) => {
      if (!block.widget) return;
      const num = numericValue(value);
      if (num == null) return;
      block.widget.config = { ...block.widget.config, value: num };
    },
    () => interacting,
  );

  onMount(() => {
    pushValue(block.id, numValue);
  });

  function onSliderInput(event: Event) {
    feedback.markEdited();
    const val = Number((event.target as HTMLInputElement).value);
    if (block.widget) {
      block.widget.config = { ...block.widget.config, value: val };
    }
    pushValue(block.id, val);
  }
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <div class="slider-container">
      <input
        type="range"
        {min}
        {max}
        {step}
        value={numValue}
        oninput={onSliderInput}
        onpointerdown={() => (dragging = true)}
        onpointerup={() => (dragging = false)}
        onpointercancel={() => (dragging = false)}
        onfocus={() => (focused = true)}
        onblur={() => (focused = false)}
        class="slider nodrag"
      />
      <span class="slider-value">{numValue}</span>
    </div>

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
    padding: 6px 10px;
    position: relative;
  }

  .slider-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .slider {
    width: 100px;
    height: 4px;
    accent-color: var(--primary);
    cursor: pointer;
  }

  .slider-value {
    font-size: 10px;
    opacity: 0.7;
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
