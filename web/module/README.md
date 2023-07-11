# Logic Mesh
A logic engine that is fully async, dynamic, and reactive written in Rust.
The engine is compiled to WebAssembly and can be used in the browser, or in a Node.js environment.

## Applications
- Low code editors
- Interactive visualizations

## Features
- Fully async and reactive
- Extensible with custom blocks, either in Rust or JavaScript when running in a WASM environment

## UI Editor
There is a sample low code editor that is built on top of Logic Mesh, which can be found [here](https://rracariu.github.io/logic-mesh/). It serves as an example of how Logic Mesh can be used, and as a simple way to experiment with the Logic Mesh engine.

## Examples

### List all registered blocks
```ts
import { initEngine } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

// List all registered blocks
const blocks = engine.listBlocks();
```

### Add block and wire them together
```ts
import { initEngine } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

// Get a command instance for the engine
const command = engine.engineCommand();

// Start the engine (this is async)
engine.run()

// Add a SineWave block
const b1 = await command.addBlock('SineWave')
// Add a StrLen block
const b2 = await command.addBlock('StrLen')

// Connect the blocks, b1 -> b2 (b1's out port to b2's in port)
command.createLink(b1, b2, 'out', 'in')
```

