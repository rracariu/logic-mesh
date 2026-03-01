<script lang="ts" generics="T">
	import { cn } from '$lib/utils';
	import { ChevronDown } from 'lucide-svelte';

	interface Props {
		options: T[];
		value?: T;
		optionLabel?: keyof T;
		placeholder?: string;
		class?: string;
		onchange?: (value: T) => void;
	}

	let {
		options,
		value = $bindable(undefined),
		optionLabel,
		placeholder = 'Select...',
		class: className,
		onchange,
	}: Props = $props();

	function getLabel(item: T): string {
		if (optionLabel && item && typeof item === 'object') {
			return String((item as Record<string, unknown>)[optionLabel as string] ?? '');
		}
		return String(item ?? '');
	}

	function handleChange(event: Event) {
		const idx = Number((event.target as HTMLSelectElement).value);
		const selected = options[idx];
		value = selected;
		onchange?.(selected);
	}

	const selectedIndex = $derived(value !== undefined ? options.indexOf(value) : -1);
</script>

<div class={cn('relative', className)}>
	<select
		class="border-input bg-background ring-offset-background focus:ring-ring w-full appearance-none rounded-md border py-2 pl-3 pr-8 text-sm focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
		value={selectedIndex >= 0 ? String(selectedIndex) : ''}
		onchange={handleChange}
	>
		{#if selectedIndex < 0}
			<option value="" disabled>{placeholder}</option>
		{/if}
		{#each options as option, i}
			<option value={String(i)}>{getLabel(option)}</option>
		{/each}
	</select>
	<ChevronDown class="text-muted-foreground pointer-events-none absolute right-2 top-2.5 h-4 w-4" />
</div>
