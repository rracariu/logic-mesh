<script lang="ts" setup>
import Accordion from 'primevue/accordion';
import AccordionTab from 'primevue/accordiontab';
import Button from 'primevue/Button';
import InputText from 'primevue/InputText';

import { Block } from '../lib/Block'
import { capitalize, computed, ref } from 'vue';

const props = defineProps<{
	blocks: Block[]
}>()

defineEmits<{
	(event: 'addBlock', block: Block): void
}>()

const blockSearch = ref('')

const blocksFiltered = computed(() => props.blocks.filter((block) => block.name.toLowerCase().includes(blockSearch.value.toLowerCase())))

const categories = computed(() => blocksFiltered.value.reduce((acc, cur) => {
	acc.add(cur.category)
	return acc
}, new Set<string>()))

const blocksForCategory = (category: string) =>
	blocksFiltered.value.filter((block) => block.category === category)


</script>


<template>
	<div class="flex flex-column w-full gap-1">
		<InputText v-model="blockSearch" placeholder="Search block..." class="w-full" />
		<Accordion :active-index="[0]" :multiple="true">
			<AccordionTab v-for="(category) in categories" :header="capitalize(category)">
				<Button v-for="block of blocksForCategory(category)" :key="block.name" :label="capitalize(block.name)"
					:title="block.doc" @click="$emit('addBlock', block)" class="m-1 w-min" text raised>
				</Button>

			</AccordionTab>
		</Accordion>
	</div>
</template>