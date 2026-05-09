<script lang="ts">
	import { tick } from 'svelte';
	import { useSvelteFlow } from '@xyflow/svelte';

	interface Props {
		/** Increment to request a re-fit. */
		trigger: number;
		padding?: number;
	}

	let { trigger, padding = 0.15 }: Props = $props();
	const { fitView } = useSvelteFlow();

	$effect(() => {
		// Re-run whenever `trigger` changes; tick() lets the new nodes mount first
		// so SvelteFlow can measure them before fitting.
		trigger;
		tick().then(() => {
			// Cap at 1× so small layouts don't get blown up to fill the viewport
			// (SvelteFlow's `maxZoom` of 4 would otherwise let fitView zoom in).
			fitView({ padding, maxZoom: 1 });
		});
	});
</script>
