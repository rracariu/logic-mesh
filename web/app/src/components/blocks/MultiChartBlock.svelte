<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { onMount, onDestroy } from 'svelte';
	import Chart from 'chart.js/auto';
	import BlockCommons from '../BlockCommons.svelte';
	import type { Block } from '$lib/Block';
	import { numericValue } from '$lib/utils';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();

	const block = $derived(data.value);
	const chartId = `multichart-${crypto.randomUUID()}`;

	const seriesKeys = ['a', 'b', 'c', 'd'] as const;
	const labelKeys = ['labelA', 'labelB', 'labelC', 'labelD'] as const;
	const colors = ['#6b9eff', '#f59e0b', '#3ecf6b', '#ef4444'];
	const MAX_POINTS = 60;

	let chart: Chart | undefined;
	const xAxis: number[] = [];
	const series: number[][] = [[], [], [], []];
	let count = 0;

	function currentLabels(): string[] {
		return labelKeys.map((k, i) => {
			const v = block.inputs[k]?.value;
			return v != null && String(v).length > 0 ? String(v) : `series ${seriesKeys[i]}`;
		});
	}

	function buildChart() {
		const ctx = document.getElementById(chartId) as HTMLCanvasElement | null;
		if (!ctx) return;
		const labels = currentLabels();
		chart = new Chart(ctx, {
			type: 'line',
			data: {
				labels: xAxis,
				datasets: series.map((data, i) => ({
					label: labels[i],
					data,
					borderColor: colors[i],
					backgroundColor: colors[i],
					fill: false,
					tension: 0.3,
					borderWidth: 1.5,
				})),
			},
			options: {
				animation: false,
				responsive: false,
				plugins: {
					legend: {
						display: true,
						position: 'bottom',
						labels: { boxWidth: 10, font: { size: 10 } },
					},
				},
				elements: { point: { radius: 0 } },
				scales: {
					x: { display: false },
					y: { ticks: { font: { size: 9 } } },
				},
			},
		});
	}

	onMount(() => {
		buildChart();
	});

	onDestroy(() => {
		chart?.destroy();
	});

	const fingerprint = $derived(
		seriesKeys.map((k) => numericValue(block.inputs[k]?.value) ?? '').join('|') +
			'/' +
			labelKeys.map((k) => block.inputs[k]?.value ?? '').join('|')
	);

	let prevFingerprint = '';
	$effect(() => {
		if (fingerprint === prevFingerprint || !chart) return;
		prevFingerprint = fingerprint;

		// Refresh legend labels on any label change.
		const labels = currentLabels();
		chart.data.datasets.forEach((ds, i) => {
			ds.label = labels[i];
		});

		xAxis.push(count++);
		if (xAxis.length > MAX_POINTS) xAxis.shift();

		seriesKeys.forEach((k, i) => {
			const num = numericValue(block.inputs[k]?.value);
			series[i].push(num == null ? NaN : num);
			if (series[i].length > MAX_POINTS) series[i].shift();
		});

		chart.update('none');
	});
</script>

<BlockCommons data={block}>
	<div class="multichart-body">
		<div class="pin-stack">
			{#each seriesKeys as key, i}
				<div class="pin-row">
					<Handle
						id={key}
						type="target"
						position={Position.Left}
						class="handle-dot handle-input"
					/>
					<span class="pin-name" style:color={colors[i]}>{key}</span>
				</div>
			{/each}
			{#each labelKeys as key}
				<div class="pin-row pin-row-label">
					<Handle
						id={key}
						type="target"
						position={Position.Left}
						class="handle-dot handle-input"
					/>
					<span class="pin-name pin-name-label">{key}</span>
				</div>
			{/each}
		</div>

		<canvas id={chartId} width="240" height="140"></canvas>
	</div>
</BlockCommons>

<style>
	.multichart-body {
		display: flex;
		align-items: flex-start;
		gap: 6px;
		padding: 4px 8px;
		position: relative;
	}

	.pin-stack {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.pin-row {
		display: flex;
		align-items: center;
		padding: 1px 8px;
		gap: 6px;
		min-height: 18px;
		position: relative;
	}

	.pin-name {
		font-size: 11px;
		font-weight: 600;
	}

	.pin-name-label {
		font-size: 10px;
		font-weight: 400;
		opacity: 0.6;
	}

	.pin-row-label {
		min-height: 14px;
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
