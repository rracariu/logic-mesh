<script lang="ts">
  import { onDestroy } from 'svelte';
  import { useSvelteFlow } from '@xyflow/svelte';
  import { RadioReceiver } from 'lucide-svelte';
  import type { Block } from '$lib/Block';
  import { blockInstances, model } from '$lib/model.svelte';
  import { blockDisplayName, findPublisher, widgetSources } from '$lib/Sources';

  interface Props {
    block: Block;
  }

  let { block }: Props = $props();

  const { getInternalNode, getZoom, setCenter } = useSvelteFlow();

  // A widget's sources are `ui` connector addresses, not links, so the
  // canvas draws no edge for them. Reading `blockInstances` (a
  // SvelteMap) and the candidates' pins keeps each publisher current as
  // blocks are added, removed or re-addressed.
  const sources = $derived.by(() => {
    const candidates = [...blockInstances.values()].map((b) => b.value);
    return widgetSources(block.widget, block.id).map((source) => ({
      ...source,
      publisher: findPublisher(source.address, candidates),
    }));
  });

  let hovered: string | undefined;

  function highlight(id: string | undefined) {
    hovered = id;
    model.highlightedBlock = id;
  }

  onDestroy(() => {
    if (hovered && model.highlightedBlock === hovered) {
      model.highlightedBlock = undefined;
    }
  });

  // Selects the source and pans to it at the current zoom.
  function reveal(id: string) {
    const node = getInternalNode(id);
    if (!node) return;
    model.currentEdge = undefined;
    model.currentBlock = model.nodes.find((n) => n.id === id);
    const { x, y } = node.internals.positionAbsolute;
    setCenter(
      x + (node.measured.width ?? 0) / 2,
      y + (node.measured.height ?? 0) / 2,
      { zoom: getZoom(), duration: 300 },
    );
  }
</script>

{#if sources.length}
  <div class="node-sources">
    {#each sources as source, i (`${i}:${source.address}`)}
      {#if source.publisher}
        {@const publisher = source.publisher}
        <button
          type="button"
          class="node-source nodrag"
          title={`${source.role} follows the values ${blockDisplayName(publisher)} publishes. Click to show it.`}
          onmouseenter={() => highlight(publisher.id)}
          onmouseleave={() => highlight(undefined)}
          onfocus={() => highlight(publisher.id)}
          onblur={() => highlight(undefined)}
          onclick={(e) => {
            // Keep the node click from selecting this widget instead.
            e.stopPropagation();
            reveal(publisher.id);
          }}
        >
          <RadioReceiver class="h-3 w-3 shrink-0" />
          <span class="node-source-role">{source.role}</span>
          <span aria-hidden="true">←</span>
          <span class="node-source-name">{blockDisplayName(publisher)}</span>
        </button>
      {:else}
        <span
          class="node-source node-source-missing"
          title={`No ExternalOut on the 'ui' connector publishes to ${source.address}, so ${source.role} never updates.`}
        >
          <RadioReceiver class="h-3 w-3 shrink-0" />
          <span class="node-source-role">{source.role}</span>
          <span aria-hidden="true">←</span>
          <span class="node-source-name">no publisher</span>
        </span>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .node-sources {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 3px 6px 4px;
    border-top: 1px dashed var(--border);
  }

  .node-source {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    max-width: 240px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--muted-foreground);
    font-family: inherit;
    font-size: 10px;
    line-height: 1.3;
    text-align: left;
    cursor: pointer;
  }

  .node-source:hover,
  .node-source:focus-visible {
    color: var(--primary);
    outline: none;
  }

  .node-source-role {
    font-weight: 600;
    flex-shrink: 0;
  }

  .node-source-name {
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* The fault red, readable in both themes. */
  .node-source-missing {
    color: #e0405b;
    cursor: default;
  }
</style>
