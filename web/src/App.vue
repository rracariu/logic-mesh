<script setup lang="ts">
import * as logic from 'logic-mesh'
import { ref } from 'vue';

const blocks = ref(logic.listBlocks())

const engine = logic.initEngine()

engine.run().then(() => console.log("Running here"))



</script>

<template>
	<div>
		<ul>
			<li v-for="block in blocks">
				{{ block.name }}
			</li>
		</ul>

	</div>
</template>