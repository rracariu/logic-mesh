import { ref } from "vue";
import { Node } from "@vue-flow/core";
import { Block } from "../lib/Block";

/**
 * Store the block nodes
 */
export const BlockNodesModel = ref<Node<Block>[]>([])