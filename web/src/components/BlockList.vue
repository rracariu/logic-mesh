<script lang="ts" setup>
import Accordion from 'primevue/accordion';
import AccordionTab from 'primevue/accordiontab';
import Button from 'primevue/button';
import InputText from 'primevue/inputtext';
import ScrollPanel from 'primevue/scrollpanel';

import type { BlockDesc } from 'logic-mesh'
import { capitalize, computed, ref } from 'vue';

const props = defineProps<{
	blocks: BlockDesc[]
}>()

defineEmits<{
	(event: 'addBlock', block: BlockDesc): void
}>()

const blockSearch = ref('')

const blocksFiltered = computed(() => props.blocks.filter((block) => block.dis.toLowerCase().includes(blockSearch.value.toLowerCase())))

const categories = computed(() => blocksFiltered.value.reduce((acc, cur) => {
	acc.add(cur.category.toLowerCase())
	return acc
}, new Set<string>()))

const blocksForCategory = (category: string) =>
	blocksFiltered.value.filter((block) => block.category.toLowerCase() === category.toLowerCase())
</script>


<template>
	<div class="flex flex-column w-full gap-1">
		<InputText v-model="blockSearch" placeholder="Search block..." class="w-full" />
		<ScrollPanel style="width: 100%; height: 90vh" class="scrollbar">
			<Accordion :multiple="true" :activeIndex="blockSearch.length > 0 ? [...Array(categories.size).keys()] : []">
				<AccordionTab v-for="category in categories" :header="capitalize(category)">
					<Button v-for="block of blocksForCategory(category)" :key="block.name" :label="capitalize(block.dis)"
						:title="block.doc" @click="$emit('addBlock', block)" class="m-1 w-min" text raised>
					</Button>
				</AccordionTab>
			</Accordion>
		</ScrollPanel>
	</div>
</template>

<style scoped>
:deep(.p-accordion-header) {
	font-size: small !important;
}

:deep(.p-accordion-header-link) {
	padding: 1.1em !important;
}

:deep(.p-accordion-content) {
	padding: 0.3em !important;
}

:deep(.p-scrollpanel.scrollbar .p-scrollpanel-wrapper) {
	border-right: 10px solid var(--surface-ground);
}

:deep(.p-scrollpanel.scrollbar .p-scrollpanel-bar) {
	background-color: var(--primary-300);
	opacity: 1;
	transition: background-color 0.3s;
}

:deep(.p-scrollpanel.scrollbar .p-scrollpanel-bar:hover) {
	background-color: var(--primary-400);
}
</style>
