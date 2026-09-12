<script lang="ts">
  import { Handle, Position } from '@xyflow/svelte';
  import { onMount, onDestroy } from 'svelte';
  import Chart from 'chart.js/auto';
  import BlockCommons from '../BlockCommons.svelte';
  import type { Block } from '$lib/Block';
  import { onValue } from '$lib/UiConnector';
  import { numericValue } from '$lib/utils';

  interface Props {
    data: { value: Block };
  }

  let { data }: Props = $props();

  const block = $derived(data.value);

  const chartId = `chart-${crypto.randomUUID()}`;
  let chart: Chart | undefined;
  const chartYAxis: number[] = [];
  const chartXAxis: number[] = [];
  const MAX_POINTS = 10;
  let count = 0;

  function buildChart() {
    const ctx = document.getElementById(chartId) as HTMLCanvasElement | null;
    if (!ctx) return;
    chart = new Chart(ctx, {
      type: 'line',
      data: {
        labels: chartXAxis,
        datasets: [{ data: chartYAxis, fill: false, tension: 0.4 }],
      },
      options: {
        animation: false,
        plugins: { legend: { display: false } },
        elements: { point: { radius: 0 } },
        scales: { y: { ticks: { stepSize: 10 } } },
      },
    });
  }

  onMount(() => {
    buildChart();
  });

  onDestroy(() => {
    chart?.destroy();
  });

  $effect(() =>
    onValue(block.id, (value) => {
      if (!chart) return;

      chartXAxis.push(count++);
      if (chartXAxis.length > MAX_POINTS) chartXAxis.shift();

      const num = numericValue(value);
      chartYAxis.push(num == null ? NaN : num);
      if (chartYAxis.length > MAX_POINTS) chartYAxis.shift();

      chart.update('none');
    }),
  );
</script>

<BlockCommons data={block}>
  <div class="pin-row pin-row-input">
    <Handle
      id="in"
      type="target"
      position={Position.Left}
      class="handle-dot handle-input"
    />
    <span class="pin-name">in</span>
  </div>

  <canvas id={chartId} width="200" height="100"></canvas>
</BlockCommons>

<style>
  .pin-row {
    display: flex;
    align-items: center;
    padding: 1px 8px;
    gap: 6px;
    min-height: 20px;
    position: relative;
  }
  .pin-row-input {
    justify-content: flex-start;
  }
  .pin-name {
    font-size: 11px;
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
