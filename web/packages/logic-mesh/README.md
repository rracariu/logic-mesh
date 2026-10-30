# Logic Mesh

A logic engine that is fully async, dynamic, and reactive written in Rust.
The engine is compiled to WebAssembly and runs in the browser (through a
bundler) and in Node.js — see [Environment support](#environment-support).

## Applications

- Low code editors
- Interactive visualizations

## Features

- Fully async and reactive
- Extensible with custom blocks, either in Rust or JavaScript when running in a WASM environment
- Protocol-agnostic connectors for external data — implement subscribe/publish/request in JavaScript and bind them to blocks with the generic `ExternalIn`, `ExternalOut`, and `Request` blocks
- Plain JavaScript functions callable as blocks via `defineJsBlocks`

## Installation

```sh
npm install logic-mesh zod
```

Upgrading from 1.x? The
[migration guide](https://github.com/rracariu/logic-mesh/blob/main/docs/migration-2.0.md#9-the-javascript-api)
lists what changed in the JavaScript API.

[Zod](https://zod.dev) 4.6 or newer is a peer dependency: `defineBlock`
describes pins with your Zod schemas, so the package must use the same `zod`
your code imports. Upgrade an app on an older Zod first: npm refuses to
install the package next to it (`ERESOLVE ... peer zod@"^4.6.0"`), even in
apps that never call `defineBlock`.

## Environment support

The package is ESM only. It ships the wasm-pack `bundler` build, whose glue
module imports the `.wasm` file directly (the WebAssembly ESM integration)
and instantiates it on first import.

### Node.js

Node.js `^22.19.0` or `>=24.5.0` loads the package with a plain `import`,
without flags or loaders:

```js
import { startEngine } from 'logic-mesh';

const { command } = startEngine();
console.log(await command.evalBlock('Add', [1, 2])); // [ 3 ]
```

- Node prints an `ExperimentalWarning` ("Importing WebAssembly module
  instances is an experimental feature"). Run
  `node --disable-warning=ExperimentalWarning` to silence it.
- From CommonJS use `await import('logic-mesh')`. `require('logic-mesh')`
  fails before Node.js 26 (`WASM is currently unsupported by require(esm)`).
- Older releases (Node.js 20, 22.12–22.18, 24.0–24.4) load the package only
  with `node --experimental-wasm-modules`, and are outside the supported
  `engines` range.

### Bundlers

- **Vite** needs [vite-plugin-wasm](https://github.com/Menci/vite-plugin-wasm).
  Without it, Vite 7 and earlier fail to build the package, and Vite 8's dev
  server serves it broken (only Vite 8's production build copes). The plugin
  instantiates the module with a top-level `await`, which the default build
  target of Vite 6 and earlier rejects, so set `build.target` too. The
  [demo app](https://github.com/rracariu/logic-mesh/blob/main/web/app/vite.config.ts)
  uses this configuration:

  ```ts
  // vite.config.ts
  import { defineConfig } from 'vite';
  import wasm from 'vite-plugin-wasm';

  export default defineConfig({
    plugins: [wasm()],
    build: {
      // Needed on Vite 6 and earlier; harmless on Vite 7 and 8.
      target: 'esnext',
    },
  });
  ```

- **webpack 5** needs `experiments: { asyncWebAssembly: true }`.

### TypeScript

Type declarations ship with the package. The declarations use
`Symbol.dispose` and `Symbol.asyncDispose`, so compile with
`skipLibCheck: true` or add `ESNext.Disposable` (or `ESNext`) to `lib`.

## UI Editor

There is a sample low code editor that is built on top of Logic Mesh, which can be found [here](https://rracariu.github.io/logic-mesh/). It serves as an example of how Logic Mesh can be used, and as a simple way to experiment with the Logic Mesh engine.

## Examples

### Start the engine and wire blocks together

`startEngine()` creates the engine, prepares the command handles, and
starts the engine's message loop — in the one order that works.

```ts
import { startEngine } from 'logic-mesh';

// Engine created and running; `command` is ready to use. The scan
// interval — how often the blocks that poll check their state — is in
// milliseconds, 200 by default.
const session = startEngine({ scanInterval: 100 });
const { command } = session;

// Add a SineWave block
const sineWaveId = await command.addBlock('SineWave');
// Add a StrLen block
const strLenId = await command.addBlock('StrLen');

// Connect the blocks, sineWave -> strLen (sineWave's out port to strLen's in port).
// A handle takes overlapping calls too: each promise settles with the
// engine's answer to its own call.
await command.createLink(sineWaveId, strLenId, 'out', 'in');

// Read a block's state and pin values, as plain objects
const block = await command.inspectBlock(strLenId);
console.log(block.state, block.outputs.out.val);

// Stop the engine; the promise resolves once it has stopped
await session.stop();
```

Values reach JavaScript in one JSON-compatible encoding, from every command,
watch notification and JS block call: a Haystack Dict is a plain object, and
Null or a missing value is `null`.

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
handle for connector attachment (`connectorCommand`), `watch()` /
`unwatch()`, and `reset()`/`stop()` that are safe to call at any point
after start and resolve once the engine has reset or stopped. Hosts that need to create the
handles early but start the engine later (say, at component mount) can
use `createEngineSession()` and call `session.start()` when ready.

### Watch for block changes

```ts
import { startEngine } from 'logic-mesh';

const session = startEngine();

// Register a callback that will be called when a block changes. The
// promise resolves with the watch once the engine opened it.
const watch = await session.watch((notification) => {
  console.log('Block changed', JSON.stringify(notification));
});

const sineWave = await session.command.addBlock('SineWave');

// Stop the notifications. `await using watch = ...` closes it at the
// end of the scope instead.
await watch.close();
```

### Handle errors

A command that fails rejects with a `LogicMeshError`: an `Error` whose
`code` names the failure — `'BlockInstanceNotFound'`,
`'UnsupportedProgramVersion'`, … (the `LogicMeshErrorCode` union) — and
whose `message` describes it. The codes are stable; the messages are
for people. The synchronous calls — `initEngine`, `registerBlock`,
`registerConnector`, `setLogLevel` — throw `LogicMeshError`s too.

```ts
import { isLogicMeshError, startEngine } from 'logic-mesh';

const { command } = startEngine();

try {
  await command.loadProgram(saved);
} catch (err) {
  if (isLogicMeshError(err) && err.code === 'UnsupportedProgramVersion') {
    console.warn('Saved by a newer version:', err.message);
  } else {
    throw err;
  }
}
```

### Logging

The engine logs warnings and errors to the console. `setLogLevel` makes
it more verbose, or silences it:

```ts
import { setLogLevel } from 'logic-mesh';

setLogLevel('debug'); // 'off' | 'error' | 'warn' | 'info' | 'debug' | 'trace'
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

A registered block runs when its inputs change: a value delivered over a
link, or one written with `writeBlockInput`, whether or not the pin is
linked.

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

### Save and load programs

`getProgram()` resolves with the engine's program in the save format, as
plain JSON: store it with `JSON.stringify` and hand it back to
`loadProgram()`. A load is all or nothing, and a program saved by a newer
release rejects with code `'UnsupportedProgramVersion'`. Programs saved by
logic-mesh 1.x load as they are.

```ts
import { startEngine, type Program } from 'logic-mesh';

const { command } = startEngine();

const program = await command.getProgram();
localStorage.setItem('program', JSON.stringify(program));

// Later, on an engine with no blocks of its own
await command.resetEngine();
await command.loadProgram(
  JSON.parse(localStorage.getItem('program')!) as Program,
);
```

A block can carry the editor's own state in `editor`, any JSON object
the engine saves back unchanged. Type it through `Program`'s parameter,
and declare the type with `type`, not `interface`:

```ts
import type { Program } from 'logic-mesh';

type MyEditor = { widget?: string; collapsed?: boolean };

const saved = (await command.getProgram()) as Program<MyEditor>;
for (const block of Object.values(saved.blocks)) {
  console.log(block.name, block.editor?.widget);
}
```

On a running engine, `command.setBlockMetadata(id, label, position)` and
`command.setBlockEditor(id, editor)` change what a block is saved with.

`ProgramSchema` is a Zod schema of the save format. It checks a program
without the engine, before you store an upload, say, and reports each
problem at its path. It reads a program as `loadProgram` does: a 1.x save
parses, and a newer format fails on its `version`. `programSchema(editor)`
also checks the editor metadata with your own schema, and types the result
with it:

```ts
import { ProgramSchema, programSchema, startEngine } from 'logic-mesh';
import { z } from 'zod';

const { command } = startEngine();

const result = ProgramSchema.safeParse(
  JSON.parse(localStorage.getItem('program')!),
);
if (!result.success) throw new Error(z.prettifyError(result.error));
await command.loadProgram(result.data);

// A loose object keeps editor keys the schema does not name.
const MyProgram = programSchema(
  z.looseObject({ widget: z.string().optional() }),
);
const saved = MyProgram.parse(await command.getProgram());
console.log(Object.values(saved.blocks)[0]?.editor?.widget);
```

A program that passes can still fail to load: only the engine knows
whether its block types are registered and its pins and links exist.
`z.toJSONSchema(ProgramSchema, { io: 'input' })` gives the format as a
JSON Schema, for tools outside JavaScript.

### List all registered blocks

```ts
import { initEngine } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

// List all registered blocks
const blocks = engine.listBlocks();
```
