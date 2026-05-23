<script lang="ts">
  import { onMount } from "svelte";
  import {
    SvelteFlow,
    Controls,
    Background,
    MiniMap,
    Panel,
    type Connection,
    type OnConnectStartParams,
  } from "@xyflow/svelte";

  import { toast } from "$lib/components/ui/sonner";
  import type { BlockNotification, BlockPin, Program } from "logic-mesh";

  import BlockNode from "../components/BlockNode.svelte";
  import ToolBar from "../components/ToolBar.svelte";
  import FitView from "../components/FitView.svelte";

  import { blockInstance, type Block } from "$lib/Block";
  import { useEngine } from "$lib/Engine";
  import { model, blockInstances } from "$lib/model.svelte";
  import { prepare, pushToEngine, save } from "$lib/Program";
  import {
    clipboardHasContent,
    clipboardRead,
    clipboardWrite,
  } from "$lib/Clipboard";

  const { engine, blocks, command, startWatch } = useEngine();

  const nodeTypes = { custom: BlockNode };

  let engineRunning = false;
  let connSource: OnConnectStartParams | undefined;
  let fitTrigger = $state(0);

  onMount(() => {
    if (!engineRunning) {
      engineRunning = true;
      engine.run();

      startWatch((notification: BlockNotification) => {
        const blockRef = blockInstances.get(notification.id);
        if (!blockRef) return;

        // Propagate the block's operational state. Mutating the
        // proxied $state field re-renders BlockCommons (red ring on
        // fault). Default to 'running' for safety if the engine
        // sends an unrecognized state.
        const nextState =
          (notification.state as
            | "running"
            | "fault"
            | "disabled"
            | "terminated") ?? "running";
        const prevState = blockRef.value.state;
        blockRef.value.state = nextState;
        blockRef.value.faultReason = notification.faultReason;

        // When source state flips between fault and not-fault, refresh
        // the edges originating from this block so the class-based
        // styling updates.
        if (prevState !== nextState) {
          const isFault = nextState === "fault";
          model.edges = model.edges.map((e) =>
            e.source === notification.id
              ? { ...e, className: isFault ? "faulted-edge" : undefined }
              : e,
          );
        }

        if (!notification.changes.length) return;

        notification.changes.forEach((change) => {
          if (change.source === "input") {
            model.edges = model.edges.map((e) =>
              e.target === blockRef.value.id && e.targetHandle === change.name
                ? { ...e, animated: true }
                : e,
            );
          }
          const pins =
            change.source === "input"
              ? blockRef.value.inputs
              : blockRef.value.outputs;
          if (pins[change.name]) {
            // Direct mutation works because blockRef.value is a $state proxy
            pins[change.name].value = change.value;
          }
        });
      });
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      // Don't intercept when the user is typing into a form control.
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }

      const mod = event.metaKey || event.ctrlKey;

      if (mod && (event.key === "c" || event.key === "C")) {
        const selected = model.nodes.filter((n) => n.selected);
        if (selected.length) {
          event.preventDefault();
          clipboardWrite(selected, model.edges);
        }
        return;
      }

      if (mod && (event.key === "v" || event.key === "V")) {
        if (clipboardHasContent()) {
          event.preventDefault();
          void pasteSelection();
        }
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  // onconnectstart in v1: (event, params) => void
  function onConnectStart(
    _event: MouseEvent | TouchEvent,
    params: OnConnectStartParams,
  ) {
    connSource = params;
  }

  async function onConnect(conn: Connection) {
    if (!connSource) return;

    if (connSource.handleType === "target") {
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
      conn.sourceHandle ?? "",
      conn.targetHandle ?? "",
    );

    if (data) {
      const link = model.edges.find(
        (e) =>
          e.target === conn.target &&
          e.source === conn.source &&
          e.sourceHandle === conn.sourceHandle &&
          e.targetHandle === conn.targetHandle,
      );
      if (link) {
        const sourceBlock = blockInstances.get(conn.source);
        if (sourceBlock) {
          const inp = sourceBlock.value.inputs[conn.sourceHandle ?? ""];
          if (inp) inp.isConnected = true;
          const out = sourceBlock.value.outputs[conn.sourceHandle ?? ""];
          if (out) out.isConnected = true;
        }
        const targetBlock = blockInstances.get(conn.target);
        if (targetBlock) {
          const inp = targetBlock.value.inputs[conn.targetHandle ?? ""];
          if (inp) inp.isConnected = true;
          const out = targetBlock.value.outputs[conn.targetHandle ?? ""];
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
    const program = save({
      name: "program",
      nodes: model.nodes,
      edges: model.edges,
    });
    const json = JSON.stringify(program, (_, value) => {
      if (typeof value === "number") return parseFloat(value.toFixed(2));
      return value;
    });
    navigator.clipboard.writeText(json);
    toast.success("Program copied to clipboard");
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

  async function pasteSelection() {
    const clip = clipboardRead();
    if (!clip) return;

    // Deselect anything that was selected so the just-pasted nodes
    // become the new selection.
    model.nodes = model.nodes.map((n) =>
      n.selected ? { ...n, selected: false } : n,
    );

    const OFFSET = 40;
    const idMap = new Map<string, string>();
    const newNodes = [];

    for (const cn of clip.nodes) {
      const newId = await command.addBlock(
        cn.desc.name,
        undefined,
        cn.desc.lib,
      );
      idMap.set(cn.originalId, newId);

      const blockValue = $state(blockInstance(newId, cn.desc));
      blockValue.label = cn.label;

      // Only restore values for inputs that were NOT connected upstream.
      // Connected inputs get their value from the recreated link (or
      // fall back to the block default when the source wasn't copied).
      for (const [name, pin] of Object.entries(cn.inputs)) {
        if (
          blockValue.inputs[name] &&
          pin.value !== undefined &&
          pin.value !== null &&
          !pin.isConnected
        ) {
          blockValue.inputs[name].value = pin.value;
          await command.writeBlockInput(newId, name, pin.value);
        }
      }

      const block = { value: blockValue };
      blockInstances.set(newId, block);

      newNodes.push({
        id: newId,
        type: "custom",
        position: { x: cn.position.x + OFFSET, y: cn.position.y + OFFSET },
        data: block,
        selected: true,
      });
    }

    const newEdges = [];
    for (const ce of clip.edges) {
      const newSource = idMap.get(ce.source);
      const newTarget = idMap.get(ce.target);
      if (!newSource || !newTarget) continue;

      const linkData = await command.createLink(
        newSource,
        newTarget,
        ce.sourceHandle,
        ce.targetHandle,
      );

      const sourceBlock = blockInstances.get(newSource);
      if (sourceBlock) {
        const out = sourceBlock.value.outputs[ce.sourceHandle];
        if (out) out.isConnected = true;
      }
      const targetBlock = blockInstances.get(newTarget);
      if (targetBlock) {
        const inp = targetBlock.value.inputs[ce.targetHandle];
        if (inp) inp.isConnected = true;
      }

      newEdges.push({
        id: crypto.randomUUID(),
        source: newSource,
        target: newTarget,
        sourceHandle: ce.sourceHandle,
        targetHandle: ce.targetHandle,
        type: "smoothstep",
        data: linkData,
      });
    }

    model.nodes = [...model.nodes, ...newNodes];
    model.edges = [...model.edges, ...newEdges];
    model.currentBlock = newNodes[0];
    model.currentEdge = undefined;
  }

  async function loadProgram(program: unknown) {
    // Phase 1 (sync): build nodes/edges from the program data and
    // register every block instance in `blockInstances` BEFORE the
    // engine receives any commands. Otherwise the first round of
    // change-of-value notifications — fired while the engine is still
    // processing the load — get dropped by the watcher callback
    // (`blockInstances.get(id)` returns undefined).
    const prog = program as Program;
    let { nodes: newNodes, edges: newEdges } = prepare(prog);

    newNodes = newNodes.map((node) => {
      const desc =
        blocks.find((b) => b.name === (node.data as { name: string }).name) ??
        (node.data as any);
      // Wrap in $state so pin value mutations are reactive
      const blockValue = $state(blockInstance(node.id, desc as any));
      const block = { value: blockValue };
      blockInstances.set(node.id, block);

      const loadedLabel = (node.data as { label?: string } | undefined)?.label;
      if (typeof loadedLabel === "string") {
        block.value.label = loadedLabel;
      }

      for (const [name, e] of Object.entries((node.data as any).inputs ?? {})) {
        const input = e as BlockPin;
        if (block.value.inputs[name]) {
          block.value.inputs[name].value = input.value;
          block.value.inputs[name].isConnected = input.isConnected;
        }
      }
      for (const [name, e] of Object.entries(
        (node.data as any).outputs ?? {},
      )) {
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
    fitTrigger += 1;

    // Phase 2 (async): now that every block instance is registered,
    // push the program into the engine.
    await pushToEngine(prog);

    toast.success("Program loaded");
  }
</script>

<div class="flex h-dvh w-dvw overflow-hidden">
  <div class="relative flex-1">
    <SvelteFlow
      bind:nodes={model.nodes}
      bind:edges={model.edges}
      {nodeTypes}
      defaultEdgeOptions={{ type: "smoothstep" }}
      minZoom={0.2}
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
      ondelete={({ nodes: deletedNodes, edges: deletedEdges }) => {
        // svelte-flow's built-in keyboard handler (Backspace +
        // Delete) removes selected nodes/edges from
        // `bind:edges`/`bind:nodes` on its own. It does not know
        // about our wasm engine, so without this callback the
        // links would visually disappear but the source block
        // would keep pushing values to the now-orphaned watch
        // channel. Forward each deletion to the engine.
        //
        // Edge id resolution: in-session edges from `onConnect`
        // have their engine link UUID on `edge.data.id`
        // (svelte-flow generates its own `edge.id`); edges
        // materialized by `prepare` from a loaded `Program` carry
        // the UUID on `edge.id` directly and leave `edge.data`
        // unset. Prefer `data.id`, fall back to `edge.id`.
        for (const edge of deletedEdges ?? []) {
          const linkId =
            (edge.data as { id?: string } | undefined)?.id ??
            (edge.id as string);
          command.removeLink(linkId);
        }
        for (const node of deletedNodes ?? []) {
          command.removeBlock(node.id);
          blockInstances.delete(node.id);
        }
        if (
          model.currentEdge &&
          deletedEdges?.some((e) => e.id === model.currentEdge!.id)
        ) {
          model.currentEdge = undefined;
        }
        if (
          model.currentBlock &&
          deletedNodes?.some((n) => n.id === model.currentBlock!.id)
        ) {
          model.currentBlock = undefined;
        }
      }}
    >
      <Background patternColor="#aaa" gap={8} />
      <Controls />
      <MiniMap class="hidden sm:block" />
      <FitView trigger={fitTrigger} />
      <Panel position="bottom-center">
        <ToolBar
          {blocks}
          onAddBlock={(desc) => model.addBlock(desc)}
          {onReset}
          {onCopy}
          {onPaste}
          {onLoad}
        />
      </Panel>
    </SvelteFlow>
  </div>
</div>
