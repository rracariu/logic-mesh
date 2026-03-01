<script lang="ts">
	import { onMount } from 'svelte';
	import {
		SvelteFlow,
		Controls,
		Background,
		MiniMap,
		Panel,
		type Connection,
		type OnConnectStartParams,
	} from '@xyflow/svelte';

	import { toast } from '$lib/components/ui/sonner';
	import type { BlockNotification, BlockPin, Program } from 'logic-mesh';

	import BlockNode from '../components/BlockNode.svelte';
	import BlockList from '../components/BlockList.svelte';
	import ToolBar from '../components/ToolBar.svelte';

	import { blockInstance, type Block } from '$lib/Block';
	import { useEngine } from '$lib/Engine';
	import { model, blockInstances } from '$lib/model.svelte';
	import { load, save } from '$lib/Program';

	const { engine, blocks, command, startWatch } = useEngine();

	const nodeTypes = { custom: BlockNode };

	let engineRunning = false;
	let connSource: OnConnectStartParams | undefined;

	onMount(() => {
		if (!engineRunning) {
			engineRunning = true;
			engine.run();

			startWatch((notification: BlockNotification) => {
				const blockRef = blockInstances.get(notification.id);
				if (!blockRef || !notification.changes.length) return;

				notification.changes.forEach((change) => {
					if (change.source === 'input') {
						model.edges = model.edges.map((e) =>
							e.target === blockRef.value.id && e.targetHandle === change.name
								? { ...e, animated: true }
								: e
						);
					}
					const pins =
						change.source === 'input' ? blockRef.value.inputs : blockRef.value.outputs;
					if (pins[change.name]) {
						// Direct mutation works because blockRef.value is a $state proxy
						pins[change.name].value = change.value;
					}
				});
			});
		}

		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.key === 'Delete') {
				if (model.currentBlock) {
					model.removeBlock(model.currentBlock.id);
					model.currentBlock = undefined;
				} else if (model.currentEdge) {
					model.removeEdgeById(model.currentEdge.id);
					command.removeLink(model.currentEdge.data?.id as string);
					model.currentEdge = undefined;
				}
			}
		};

		window.addEventListener('keydown', handleKeyDown);
		return () => window.removeEventListener('keydown', handleKeyDown);
	});

	// onconnectstart in v1: (event, params) => void
	function onConnectStart(_event: MouseEvent | TouchEvent, params: OnConnectStartParams) {
		connSource = params;
	}

	async function onConnect(conn: Connection) {
		if (!connSource) return;

		if (connSource.handleType === 'target') {
			conn = {
				source: conn.target,
				target: conn.source,
				sourceHandle: conn.targetHandle,
				targetHandle: conn.sourceHandle,
			};
		}
		connSource = undefined;

		const data = await command.createLink(
			conn.source,
			conn.target,
			conn.sourceHandle ?? '',
			conn.targetHandle ?? ''
		);

		if (data) {
			const link = model.edges.find(
				(e) =>
					e.target === conn.target &&
					e.source === conn.source &&
					e.sourceHandle === conn.sourceHandle &&
					e.targetHandle === conn.targetHandle
			);
			if (link) {
				const sourceBlock = blockInstances.get(conn.source);
				if (sourceBlock) {
					const inp = sourceBlock.value.inputs[conn.sourceHandle ?? ''];
					if (inp) inp.isConnected = true;
					const out = sourceBlock.value.outputs[conn.sourceHandle ?? ''];
					if (out) out.isConnected = true;
				}
				const targetBlock = blockInstances.get(conn.target);
				if (targetBlock) {
					const inp = targetBlock.value.inputs[conn.targetHandle ?? ''];
					if (inp) inp.isConnected = true;
					const out = targetBlock.value.outputs[conn.targetHandle ?? ''];
					if (out) out.isConnected = true;
				}
				link.data = data;
				model.edges = [...model.edges]; // trigger $state.raw update
			}
		}
	}

	async function onReset() {
		await command.resetEngine();
		model.clearAll();
	}

	function onCopy() {
		const program = save({ name: 'program', nodes: model.nodes, edges: model.edges });
		const json = JSON.stringify(program, (_, value) => {
			if (typeof value === 'number') return parseFloat(value.toFixed(2));
			return value;
		});
		navigator.clipboard.writeText(json);
		toast.success('Program copied to clipboard');
	}

	function onPaste() {
		onReset()
			.then(async () => {
				const clipText = await navigator.clipboard.readText();
				await loadProgram(JSON.parse(clipText));
			})
			.catch((err) => toast.error(`Paste failed: ${err}`));
	}

	function onLoad(program: Program) {
		onReset()
			.then(async () => await loadProgram(program))
			.catch((err) => toast.error(`Load failed: ${err}`));
	}

	async function loadProgram(program: unknown) {
		let { nodes: newNodes, edges: newEdges } = await load(program as Program);

		newNodes = newNodes.map((node) => {
			const desc =
				blocks.find((b) => b.name === (node.data as { name: string }).name) ??
				(node.data as any);
			// Wrap in $state so pin value mutations are reactive
			const blockValue = $state(blockInstance(node.id, desc as any));
			const block = { value: blockValue };
			blockInstances.set(node.id, block);

			for (const [name, e] of Object.entries((node.data as any).inputs ?? {})) {
				const input = e as BlockPin;
				if (block.value.inputs[name]) {
					block.value.inputs[name].value = input.value;
					block.value.inputs[name].isConnected = input.isConnected;
				}
			}
			for (const [name, e] of Object.entries((node.data as any).outputs ?? {})) {
				const output = e as BlockPin;
				if (block.value.outputs[name]) {
					block.value.outputs[name].value = output.value;
				}
			}

			node.data = block;
			return node;
		});

		model.nodes = [...model.nodes, ...newNodes];
		model.edges = [...model.edges, ...newEdges];
		toast.success('Program loaded');
	}
</script>

<div class="flex h-screen w-screen overflow-hidden">
	<!-- Left panel: block list -->
	<div class="w-[18%] min-w-[160px] overflow-hidden border-r bg-background">
		<BlockList {blocks} onAddBlock={(desc) => model.addBlock(desc)} />
	</div>

	<!-- Right panel: flow editor -->
	<div class="relative flex-1">
		<SvelteFlow
			bind:nodes={model.nodes}
			bind:edges={model.edges}
			{nodeTypes}
			defaultEdgeOptions={{ type: 'smoothstep' }}
			minZoom={1}
			maxZoom={4}
			elevateEdgesOnSelect={true}
			onconnect={onConnect}
			onconnectstart={onConnectStart}
			onnodeclick={({ node }) => {
				model.currentEdge = undefined;
				model.currentBlock = node;
			}}
			onedgeclick={({ edge }) => {
				model.currentBlock = undefined;
				model.currentEdge = edge;
			}}
		>
			<Background patternColor="#aaa" gap={8} />
			<Controls />
			<MiniMap />
			<Panel position="bottom-center">
				<ToolBar {onReset} {onCopy} {onPaste} {onLoad} />
			</Panel>
		</SvelteFlow>
	</div>
</div>
