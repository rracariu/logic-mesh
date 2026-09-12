# Logic Mesh

A logic engine that is fully async, dynamic, and reactive written in Rust.
The engine is compiled to WebAssembly and can be used in the browser, or in a Node.js environment.

## Applications

- Low code editors
- Interactive visualizations

## Features

- Fully async and reactive
- Extensible with custom blocks, either in Rust or JavaScript when running in a WASM environment
- Protocol-agnostic connectors for external data — implement subscribe/publish/request in JavaScript and bind them to blocks with the generic `ExternalIn`, `ExternalOut`, and `Request` blocks
- Plain JavaScript functions callable as blocks via `defineJsBlocks`

## UI Editor

There is a sample low code editor that is built on top of Logic Mesh, which can be found [here](https://rracariu.github.io/logic-mesh/). It serves as an example of how Logic Mesh can be used, and as a simple way to experiment with the Logic Mesh engine.

## Examples

### Start the engine and wire blocks together

`startEngine()` creates the engine, prepares the command handles, and
starts the engine's message loop — in the one order that works.

```ts
import { startEngine } from 'logic-mesh';

// Engine created and running; `command` is ready to use
const { command } = startEngine();

// Add a SineWave block
const sineWaveId = await command.addBlock('SineWave');
// Add a StrLen block
const strLenId = await command.addBlock('StrLen');

// Connect the blocks, sineWave -> strLen (sineWave's out port to strLen's in port).
// Await each command before issuing the next one on the same handle:
// a handle supports one in-flight command at a time.
await command.createLink(sineWaveId, strLenId, 'out', 'in');
```

Under the hood the sequence matters, and getting it wrong fails in
confusing ways — which is why the wrapper owns it:

```ts
import { initEngine } from 'logic-mesh';

const engine = initEngine();

// Command handles must be created before `run()`: `engineCommand()`
// borrows the engine mutably, and `run()`'s future holds that borrow
// from its first poll onward ("recursive use of an object detected").
const command = engine.engineCommand();

// Start the engine without awaiting it: commands are only serviced by
// the running engine's message loop, and `run()`'s promise resolves
// only on shutdown — awaiting it first deadlocks.
engine.run();
```

`startEngine` returns an `EngineSession`, which also carries a second
handle for connector attachment (`connectorCommand`), a `watch()`
method backed by pre-created handles, and `reset()`/`stop()` that are
safe to call at any point after start. Hosts that need to create the
handles early but start the engine later (say, at component mount) can
use `createEngineSession()` and call `session.start()` when ready.

### Watch for block changes

```ts
import { startEngine } from 'logic-mesh';

const session = startEngine();

// Register a callback that will be called when a block changes. Each
// watch permanently occupies one command handle; the session
// pre-creates them (one by default — see the `watchSlots` option).
session.watch((notification) => {
  console.log('Block changed', JSON.stringify(notification));
});

const sineWave = await session.command.addBlock('SineWave');
```

### Expose JavaScript functions as blocks

`defineJsBlocks` turns plain JS functions — sync or async — into
engine blocks. Each function becomes addressable through a `Request`
block: the block's `in` pin carries the argument, `out` carries the
returned (and awaited) result, and a rejection surfaces as the block's
fault. The `Request` block's `timeout` pin bounds functions that never
settle.

```ts
import { defineJsBlocks, startEngine } from 'logic-mesh';

const session = startEngine();

const jsBlocks = defineJsBlocks({
  scale: (value) => (value as number) * 2,
  fetchTemp: async (zone) => {
    const response = await fetch(`/api/temp/${zone as string}`);
    return response.json();
  },
});

// Register the backing connector and attach it to the running engine
await jsBlocks.attach(session.command);

// Add a block that calls `scale` — a `Request` block wired to it
const id = await jsBlocks.addBlock(session.command, 'scale');
await session.command.writeBlockInput(id, 'in', 21); // out becomes 42
```

Functions taking several inputs receive them as one object: write a
dict value to the `in` pin and the function is handed a plain JS
object (nested dicts included). `jsBlocks.detach(command)` removes the
connector again; after an engine `reset()` (which detaches all managed
connectors) calling `attach` once more restores it.

### Add a custom JavaScript block

For blocks with typed, named pins that show up in the block library
like the built-ins do, register a full block description instead:

```ts
import { createEngineSession, type BlockDesc, type JsBlock } from 'logic-mesh';

// Blocks must be registered before the engine starts, so create the
// session first and start it after registration
const session = createEngineSession();

/**
 * Defines a block that is implemented in JS
 */
const JsAddBlock = {
  // The block description
  desc: {
    name: 'AddBlock',
    dis: 'JS Add block',
    lib: 'examples',
    ver: '0.0.1',
    category: 'Docs',
    doc: 'Adds two numbers',
    implementation: 'external',
    inputs: [
      { name: 'in1', kind: 'number' },
      { name: 'in2', kind: 'number' },
    ],
    outputs: [{ name: 'out', kind: 'number' }],
  } satisfies BlockDesc,
  // The block factory: called once per block instance, it returns the
  // execute function. Inputs and outputs are arrays in pin order.
  executor: () => async (inputs: unknown[]) => {
    return [Number(inputs[0]) + Number(inputs[1])];
  },
} satisfies JsBlock;

session.engine.registerBlock(JsAddBlock.desc, JsAddBlock.executor);

session.start();
```

The executor may be re-invoked with the same inputs if the engine
interrupts a call mid-flight (at-least-once), so side-effecting
executors should be idempotent.

Note the trigger trap: a reactive registered block (the default
`runCondition: 'change'`) fires on link-delivered values — a bare
`writeBlockInput` on its pins does not trigger execution. Use
`runCondition: 'always'` or drive the pin over a link.

### Which extension point to use

The three JS extension points overlap on purpose — each trades ergonomics
for capability at a different spot. Pick by the shape of what you are adding:

| You are adding…                                                                                     | Use                                          | Because                                                                                                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A function the graph calls with one value (a transform, a lookup, a service call)                   | `defineJsBlocks`                             | One `in` → one `out` through a `Request` block: attach/detach on a **running** engine, a `timeout` pin, and a cancelled call is retried automatically (at-least-once — make handlers idempotent). Several inputs travel as one object on the single `in` pin.                                                                    |
| A real block: several **independently wired** input pins, typed pins, an entry in the block library | `registerBlock` / `defineBlock` (TypedBlock) | Only registered blocks get named pins that separate upstream blocks can each link to, per-pin kinds (defaults come from `defineBlock`'s schema layer — raw `registerBlock` pins carry name and kind only), and a catalog entry. Register **before** `session.start()` — block registration is not available on a running engine. |
| A stream of values pushed _into_ the graph, a sink, or a whole protocol (MQTT, WebSocket, …)        | a custom connector                           | `subscribe`/`publish`/`request` with engine-managed lifecycle; `ExternalIn`/`ExternalOut`/`Request` blocks bind to it by name and address, so the binding is data and serializes with the program. `defineJsBlocks` is itself a connector specialized to the request case.                                                       |

Rule of thumb: start with `defineJsBlocks`; move to `registerBlock` the moment
you need a second independently-linked input pin or a library entry; drop to a
custom connector when values flow on their own schedule rather than
per-request.

### Add a custom JavaScript connector

Connectors bridge the engine to external systems (MQTT, WebSockets, HTTP, …).
A connector implements `subscribe`, `publish`, and `request` (plus optional
`start`/`stop`); the generic `ExternalIn`/`ExternalOut`/`Request` blocks bind
to it by connector name and address.

```ts
import { registerConnector, startEngine, type JsConnector } from 'logic-mesh';

const connector = {
  subscribe(address, callback) {
    // Push values with callback(value); report an error with
    // callback(undefined, detail); end the stream with callback().
    const timer = setInterval(() => callback(Math.random() * 100), 1000);
    // Return an unsubscribe function (or a Promise of one)
    return () => clearInterval(timer);
  },
  publish(address, value) {
    console.log(`publish ${value} to ${address}`);
  },
  request(address, value) {
    return Promise.resolve({ echo: value });
  },
} satisfies JsConnector;

// Put the connector in the process-wide registry
registerConnector('demo', connector);

const { command } = startEngine();

// ExternalIn streams subscribed values into the graph via its `out` pin
const inputId = await command.addBlock('ExternalIn');
await command.writeBlockInput(inputId, 'connector', 'demo');
await command.writeBlockInput(inputId, 'address', 'sensor/1');

// ExternalOut publishes whatever is wired to its `in` pin
const outputId = await command.addBlock('ExternalOut');
await command.writeBlockInput(outputId, 'connector', 'demo');
await command.writeBlockInput(outputId, 'address', 'actuator/1');

// Attach the connector to the running engine; the engine manages its
// lifecycle from here (it is stopped and detached on engine reset)
await command.addConnector('demo');
```

### List all registered blocks

```ts
import { initEngine } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

// List all registered blocks
const blocks = engine.listBlocks();
```
