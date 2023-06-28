import { GraphEdge, GraphNode } from "@vue-flow/core";
import { ref } from "vue";

export const currentBlock = ref<GraphNode<any, any, string>>();
export const currentLink = ref<GraphEdge>();