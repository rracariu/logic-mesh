<script setup lang="ts">
import { Block } from '../../lib/Block';
import BlockNode from '../BlockNode.vue';
import ChartBlock from './ChartBlock.vue';
import CheckboxBlock from './CheckboxBlock.vue';
import InputBlock from './InputBlock.vue';

defineProps<{ data: Block }>()

</script>

<template>
	<BlockNode v-if="data.desc.implementation == 'native'" :data="data" />
	<InputBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Input'" :data="data" />
	<CheckboxBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Checkbox'" :data="data" />
	<ChartBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Chart'" :data="data" />
	<BlockNode v-else :data="data" />
</template>