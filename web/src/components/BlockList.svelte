<script lang="ts">
	import type { BlockDesc } from 'logic-mesh';
	import {
		Accordion,
		AccordionContent,
		AccordionItem,
		AccordionTrigger
	} from '$lib/components/ui/accordion';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';

	interface Props {
		blocks: BlockDesc[];
		onAddBlock: (block: BlockDesc) => void;
	}

	let { blocks, onAddBlock }: Props = $props();

	let blockSearch = $state('');
	let manualOpen = $state<string[]>([]);

	const blocksFiltered = $derived(
		blocks.filter((b) => b.dis.toLowerCase().includes(blockSearch.toLowerCase()))
	);

	const categories = $derived(
		[...new Set(blocksFiltered.map((b) => b.category.toLowerCase()))]
	);

	const openItems = $derived(blockSearch.length > 0 ? categories : manualOpen);

	function blocksForCategory(category: string) {
		return blocksFiltered.filter((b) => b.category.toLowerCase() === category);
	}

	function capitalize(s: string) {
		return s.charAt(0).toUpperCase() + s.slice(1);
	}
</script>

<div class="flex w-full flex-col gap-1 p-1">
	<Input bind:value={blockSearch} placeholder="Search block..." class="w-full" />
	<div class="h-[90vh] overflow-y-auto pr-1">
		<Accordion
			type="multiple"
			value={openItems}
			onValueChange={(v) => {
				if (!blockSearch.length) manualOpen = v;
			}}
		>
			{#each categories as category}
				<AccordionItem value={category}>
					<AccordionTrigger>{capitalize(category)}</AccordionTrigger>
					<AccordionContent>
						<div class="flex flex-wrap gap-1 py-1">
							{#each blocksForCategory(category) as block (block.name)}
								<Button
									variant="outline"
									size="sm"
									title={block.doc}
									onclick={() => onAddBlock(block)}
									class="text-xs"
								>
									{capitalize(block.dis)}
								</Button>
							{/each}
						</div>
					</AccordionContent>
				</AccordionItem>
			{/each}
		</Accordion>
	</div>
</div>
