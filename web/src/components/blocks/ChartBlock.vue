<script setup lang="ts">
import Chart from 'chart.js/auto';

import { Handle, Position, } from '@vue-flow/core';
import { capitalize, onMounted, watch } from 'vue';

import { Block } from '../../lib/Block';

const props = defineProps<{ data: Block }>()

let chart: Chart;
let chartYAxis = [] as number[]
let chartXAxis = [] as number[]
let count = 0

const draw = () => {
	chart = new Chart(document.getElementById('chart') as HTMLCanvasElement, {
		type: 'line',
		data: {
			labels: chartXAxis,
			datasets: [
				{
					data: chartYAxis,
					fill: false,
					tension: 0.4
				},

			]
		},
		options: {
			animation: false,
			plugins: {
				legend: {
					display: false
				}
			},
			elements: {
				point: {
					radius: 0
				}
			},
			scales: {
				y: {
					ticks: {
						stepSize: 10
					}
				}
			}
		}
	});
}

onMounted(() => {
	draw()
});

watch(() => props.data.inputs.in.value, () => {
	chart.destroy()
	draw()

	chartXAxis.push(count++)
	chartXAxis = chartXAxis.slice(-10)

	const curVal = props.data.inputs.in.value as number;
	chartYAxis.push(curVal)
	chartYAxis = chartYAxis.slice(-10)
})

</script>

<template>
	<div class="flex">
		<Handle :id="Object.keys(data.inputs)[0] ?? 'in'" type="target" :position="Position.Left" class="handle-input" />

		<div class="flex align-items-center justify-content-center m-1 border-round">
			{{ capitalize(Object.keys(data.inputs)[0] ?? '') }}
		</div>


		<canvas id="chart" width="200" height="100" />

	</div>
</template>

<style scoped>
[class*="handle-"] {
	padding: 1px;
	display: inline-table;
	height: 1em;
	border-radius: 10%;
	overflow: hidden;
}

.handle-input {
	background: var(--blue-200);
}
</style> 