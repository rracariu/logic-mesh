import type { Edge, Node } from '@xyflow/svelte';
import type { BlockDesc } from 'logic-mesh';
import type { Block } from './Block';

interface ClipNode {
  originalId: string;
  desc: BlockDesc;
  label: string;
  position: { x: number; y: number };
  inputs: Record<string, { value: unknown; isConnected: boolean }>;
}

interface ClipEdge {
  source: string;
  target: string;
  sourceHandle: string;
  targetHandle: string;
}

let clipboard: { nodes: ClipNode[]; edges: ClipEdge[] } | null = null;

export function clipboardHasContent(): boolean {
  return clipboard !== null && clipboard.nodes.length > 0;
}

/**
 * Snapshot the given nodes plus the subset of edges that interconnect them.
 * Inter-block edges anchored to non-snapshotted nodes are dropped — they
 * have no remappable counterpart on paste.
 */
export function clipboardWrite(nodes: Node[], edges: Edge[]) {
  if (!nodes.length) {
    clipboard = null;
    return;
  }

  const ids = new Set(nodes.map((n) => n.id));

  clipboard = {
    nodes: nodes.map((n) => {
      const block = (n.data as { value: Block }).value;
      const inputs: ClipNode['inputs'] = {};
      for (const [name, pin] of Object.entries(block.inputs)) {
        inputs[name] = { value: pin.value, isConnected: !!pin.isConnected };
      }
      return {
        originalId: n.id,
        desc: block.desc,
        label: block.label,
        position: { x: n.position.x, y: n.position.y },
        inputs,
      };
    }),
    edges: edges
      .filter((e) => ids.has(e.source) && ids.has(e.target))
      .map((e) => ({
        source: e.source,
        target: e.target,
        sourceHandle: e.sourceHandle ?? '',
        targetHandle: e.targetHandle ?? '',
      })),
  };
}

export function clipboardRead() {
  return clipboard;
}
