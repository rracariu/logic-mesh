<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { onMount } from 'svelte';
  import { Input } from '$lib/components/ui/input';
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

  // Feedback tracks the source while the input is not focused; a user
  // edit wins during interaction and is never echoed back by feedback.
  let interacting = $state(false);
  const feedback = useValueFeedback(
    () => block.widget,
    (value) => {
      if (value == null || !block.widget) return;
      block.widget.config = { ...block.widget.config, value: String(value) };
    },
    () => interacting,
  );

  onMount(() => {
    if (config.value != null) {
      pushValue(block.id, config.value);
    }
  });

  function onInputChange(event: Event) {
    feedback.markEdited();
    const val = (event.target as HTMLInputElement).value;
    if (block.widget) {
      block.widget.config = { ...block.widget.config, value: val };
    }
    pushValue(block.id, val);
  }
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Input
      value={String(config.value ?? '')}
      oninput={onInputChange}
      onfocus={() => (interacting = true)}
      onblur={() => (interacting = false)}
      class="h-7 w-28 text-xs"
    />
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
