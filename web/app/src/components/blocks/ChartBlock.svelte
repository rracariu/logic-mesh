<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { onMount } from 'svelte';
	import Chart from 'chart.js/auto';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);
	const inputKey = $derived(Object.keys(block.inputs)[0] ?? 'in');

	const chartId = `chart-${crypto.randomUUID()}`;
	let chart: Chart;
	let chartYAxis: number[] = [];
	let chartXAxis: number[] = [];
	let count = 0;

	function draw() {
		chart = new Chart(document.getElementById(chartId) as HTMLCanvasElement, {
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
		draw();
	});

	let prevInputValue: unknown;
	$effect(() => {
		const curVal = block.inputs.in.value;
		if (curVal !== prevInputValue) {
			prevInputValue = curVal;
			if (chart) {
				chart.destroy();
				draw();

				chartXAxis.push(count++);
				chartXAxis = chartXAxis.slice(-10);

				chartYAxis.push(curVal as number);
				chartYAxis = chartYAxis.slice(-10);
			}
		}
	});
</script>

<BlockCommons data={block}>
	<div class="pin-row pin-row-input">
		<Handle id={inputKey} type="target" position={Position.Left} class="handle-dot handle-input" />
		<span class="pin-name">{inputKey}</span>
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
	.pin-row-input { justify-content: flex-start; }
	.pin-name { font-size: 11px; }

	:global(.handle-dot) {
		width: 8px !important;
		height: 8px !important;
		border-radius: 50% !important;
		min-width: 0 !important;
		border: 1.5px solid white !important;
	}
	:global(.handle-input) { background: #6b9eff !important; }
</style>
