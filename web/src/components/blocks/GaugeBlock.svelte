<script lang="ts">
	import { Handle, Position } from '@xyflow/svelte';
	import { Plus, Minus } from 'lucide-svelte';
	import { Button } from '$lib/components/ui/button';
	import BlockCommons from '../BlockCommons.svelte';
	import { useEngine } from '$lib/Engine';
	import type { Block } from '$lib/Block';

	interface Props {
		data: { value: Block };
	}

	let { data }: Props = $props();
	const { command } = useEngine();

	const block = $derived(data.value);
	const inputKey = $derived(Object.keys(block.inputs)[0] ?? 'in');
	const outputKey = $derived(Object.keys(block.outputs)[0] ?? 'out');

	const numValue = $derived(Number(block.inputs.in.value ?? 0));

	// SVG knob parameters
	const radius = 36;
	const cx = 44;
	const cy = 44;
	const minAngle = -225;
	const maxAngle = 45;
	const minVal = 0;
	const maxVal = 100;

	const angle = $derived(
		minAngle + ((numValue - minVal) / (maxVal - minVal)) * (maxAngle - minAngle)
	);

	function toXY(angleDeg: number, r: number) {
		const rad = ((angleDeg - 90) * Math.PI) / 180;
		return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
	}

	const thumbPos = $derived(toXY(angle, radius - 4));
	const arcStart = $derived(toXY(minAngle, radius));
	const arcEnd = $derived(toXY(angle, radius));
	const largeArc = $derived(angle - minAngle > 180 ? 1 : 0);

	function updateOutput(newVal: number) {
		block.inputs.in.value = newVal;
		block.outputs.out.value = newVal;
		command.writeBlockOutput(block.id, outputKey, newVal);
	}

	function increment() {
		updateOutput(numValue + 1);
	}

	function decrement() {
		updateOutput(numValue - 1);
	}
</script>

<BlockCommons data={block}>
	<Handle id={inputKey} type="target" position={Position.Left} class="handle-input" />

	<div class="flex flex-col items-center gap-1 p-1">
		<span class="text-xs capitalize">{inputKey}</span>

		<svg width="88" height="88" viewBox="0 0 88 88">
			<!-- Track -->
			<circle cx={cx} cy={cy} r={radius} fill="none" stroke="var(--muted)" stroke-width="6" />
			<!-- Value arc -->
			<path
				d="M {arcStart.x} {arcStart.y} A {radius} {radius} 0 {largeArc} 1 {arcEnd.x} {arcEnd.y}"
				fill="none"
				stroke="var(--primary)"
				stroke-width="6"
				stroke-linecap="round"
			/>
			<!-- Thumb -->
			<circle cx={thumbPos.x} cy={thumbPos.y} r="4" fill="var(--primary)" />
			<!-- Value label -->
			<text x={cx} y={cy + 5} text-anchor="middle" font-size="12" fill="var(--foreground)">
				{numValue}
			</text>
		</svg>

		<div class="flex gap-1">
			<Button variant="ghost" size="icon" class="h-6 w-6" onclick={decrement} aria-label="Decrement">
				<Minus class="h-3 w-3" />
			</Button>
			<Button variant="ghost" size="icon" class="h-6 w-6" onclick={increment} aria-label="Increment">
				<Plus class="h-3 w-3" />
			</Button>
		</div>

		<span class="text-xs capitalize">{outputKey}</span>
	</div>

	<Handle id={outputKey} type="source" position={Position.Right} class="handle-output" />
</BlockCommons>

<style>
	:global(.handle-input) {
		background: #93c5fd !important;
	}
	:global(.handle-output) {
		background: #86efac !important;
	}
</style>
