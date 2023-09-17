<script setup lang="ts">
import { Block } from '../lib/Block';
import BlockNode from './blocks/GenericBlock.vue';
import ChartBlock from './blocks/ChartBlock.vue';
import CheckboxBlock from './blocks/CheckboxBlock.vue';
import InputBlock from './blocks/InputBlock.vue';
import GaugeBlock from './blocks/GaugeBlock.vue';

defineProps<{ data: Block }>()

</script>

<template>
	<BlockNode v-if="data.desc.implementation == 'native'" :data="data" />
	<InputBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Input'" :data="data" />
	<CheckboxBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Checkbox'" :data="data" />
	<ChartBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Chart'" :data="data" />
	<GaugeBlock v-else-if="data.desc.implementation == 'external' && data.desc.name == 'Gauge'" :data="data" />
	<BlockNode v-else :data="data" />
</template>