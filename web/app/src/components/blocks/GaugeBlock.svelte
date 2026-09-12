<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { onValue } from '$lib/UiConnector';
  import { numericValue } from '$lib/utils';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);

  let raw = $state<unknown>(undefined);
  $effect(() => onValue(block.id, (v) => (raw = v)));

  const numValue = $derived(numericValue(raw) ?? 0);

  // SVG knob parameters
  const radius = 36;
  const cx = 44;
  const cy = 44;
  const minAngle = -225;
  const maxAngle = 45;
  const minVal = 0;
  const maxVal = 100;

  const angle = $derived(
    minAngle +
      ((numValue - minVal) / (maxVal - minVal)) * (maxAngle - minAngle),
  );

  function toXY(angleDeg: number, r: number) {
    const rad = ((angleDeg - 90) * Math.PI) / 180;
    return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
  }

  const thumbPos = $derived(toXY(angle, radius - 4));
  const arcStart = $derived(toXY(minAngle, radius));
  const arcEnd = $derived(toXY(angle, radius));
  const largeArc = $derived(angle - minAngle > 180 ? 1 : 0);
</script>

<BlockCommons data={block}>
  <div class="ui-block-body">
    <Handle
      id="in"
      type="target"
      position={Position.Left}
      class="handle-dot handle-input"
    />

    <div class="flex flex-col items-center gap-1">
      <svg width="88" height="88" viewBox="0 0 88 88">
        <circle
          {cx}
          {cy}
          r={radius}
          fill="none"
          stroke="var(--muted)"
          stroke-width="6"
        />
        <path
          d="M {arcStart.x} {arcStart.y} A {radius} {radius} 0 {largeArc} 1 {arcEnd.x} {arcEnd.y}"
          fill="none"
          stroke="var(--primary)"
          stroke-width="6"
          stroke-linecap="round"
        />
        <circle cx={thumbPos.x} cy={thumbPos.y} r="4" fill="var(--primary)" />
        <text
          x={cx}
          y={cy + 5}
          text-anchor="middle"
          font-size="12"
          fill="var(--foreground)"
        >
          {numValue}
        </text>
      </svg>
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
  :global(.handle-output) {
    background: #6bcf7f !important;
  }
</style>
