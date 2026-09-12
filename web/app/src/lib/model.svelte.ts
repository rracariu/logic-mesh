import type { Edge, Node } from '@xyflow/svelte';
import type { BlockDesc } from 'logic-mesh';
import { SvelteMap } from 'svelte/reactivity';
import { blockInstance, cloneWidget, type Block } from './Block';
import { useEngine } from './Engine';
import { forgetBlockAddress, UI_CONNECTOR_NAME } from './UiConnector';
import { isWidgetDesc } from './Widgets';

const { command, blocks } = useEngine();

// Registry of live block instances keyed by node id. `SvelteMap` rather
// than a plain `Map` so the registry itself is a reactive source —
// callers that iterate or check membership inside `$derived`/`$effect`
// re-run when blocks are added/removed.
export const blockInstances = new SvelteMap<string, { value: Block }>();

/**
 * Central reactive model for the flow.
 *
 * Uses a class so that properties can be freely reassigned from any module.
 * @xyflow/svelte v1 requires $state.raw for nodes/edges (immutable array replacements).
 */
class FlowModel {
  nodes = $state.raw<Node[]>([]);
  edges = $state.raw<Edge[]>([]);
  currentBlock = $state<Node | undefined>(undefined);
  currentEdge = $state<Edge | undefined>(undefined);

  async addBlock(desc: BlockDesc): Promise<Block> {
    // A widget palette entry is realized as an engine ExternalIn (input
    // widgets) or ExternalOut (display widgets) bound to the 'ui'
    // connector, addressed by its own block id.
    const widget = isWidgetDesc(desc) ? desc.widget : undefined;
    const engineDesc = widget
      ? blocks.find(
          (b) =>
            b.name ===
              (widget.direction === 'in' ? 'ExternalIn' : 'ExternalOut') &&
            b.lib === 'core',
        )
      : desc;
    if (!engineDesc) {
      throw new Error(`No engine block for widget '${desc.name}'`);
    }

    const id = await command.addBlock(
      engineDesc.name,
      undefined,
      engineDesc.lib,
    );
    // Wrap in $state so pin value mutations are reactive
    const blockValue = $state(blockInstance(id, engineDesc));
    const block = { value: blockValue };

    if (widget) {
      // Deep-cloned so the placed block's config never aliases the
      // shared palette default (a shallow copy would leave MultiChart's
      // `series` array and its element objects shared by every
      // instance placed from the palette).
      blockValue.widget = cloneWidget({
        kind: widget.kind,
        config: widget.defaultConfig ?? {},
      });
      blockValue.inputs['connector'].value = UI_CONNECTOR_NAME;
      blockValue.inputs['address'].value = id;
      // Sequential awaits: EngineCommand methods take `&mut self`, so
      // overlapping calls on the shared instance panic in wasm-bindgen.
      await command.writeBlockInput(id, 'connector', UI_CONNECTOR_NAME);
      await command.writeBlockInput(id, 'address', id);
    }

    const position = this.currentBlock
      ? {
          x:
            (this.currentBlock.position.x || 0) +
            (this.currentBlock.measured?.width ?? 200) +
            50,
          y: this.currentBlock.position.y || 0,
        }
      : { x: 250, y: 5 };

    this.nodes = [...this.nodes, { id, type: 'custom', position, data: block }];
    blockInstances.set(id, block);
    this.currentBlock = this.nodes.find((n) => n.id === id);

    return block.value;
  }

  removeBlock(id: string) {
    const block = blockInstances.get(id);
    this.nodes = this.nodes.filter((n) => n.id !== id);
    this.edges = this.edges.filter((e) => e.source !== id && e.target !== id);
    command.removeBlock(id);
    blockInstances.delete(id);
    if (block) {
      forgetBlockAddress(block.value);
    }
  }

  removeEdgeById(id: string) {
    this.edges = this.edges.filter((e) => e.id !== id);
  }

  clearAll() {
    this.nodes = [];
    this.edges = [];
    blockInstances.clear();
    this.currentBlock = undefined;
    this.currentEdge = undefined;
  }
}

export const model = new FlowModel();
