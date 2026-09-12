<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { pushValue } from '$lib/UiConnector';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);

  let pressed = $state(false);

  onMount(() => {
    pushValue(block.id, false);
  });

  function onPress() {
    pressed = true;
    pushValue(block.id, true);
  }

  function onRelease() {
    if (!pressed) return;
    pressed = false;
    pushValue(block.id, false);
  }
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Button
      variant="outline"
      size="sm"
      class="h-7 text-xs"
      onpointerdown={onPress}
      onpointerup={onRelease}
      onpointerleave={onRelease}
    >
      {pressed ? 'ON' : 'OFF'}
    </Button>
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
