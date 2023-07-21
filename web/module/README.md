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

// Add a SineWave block
const sineWaveId = await command.addBlock('SineWave')
// Add a StrLen block
const strLenId = await command.addBlock('StrLen')

// Connect the blocks, sineWave -> strLen (sineWave's out port to strLen's in port)
command.createLink(sineWaveId, strLenId, 'out', 'in')

// Start the engine (this is async)
engine.run()
```

### Watch for block changes
```ts
import { initEngine } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

// Get a command instance for the engine
const command = engine.engineCommand();

// Add a SineWave block
const sineWave = await command.addBlock('SineWave')

// Create a new command instance for the watcher
const watchCommand = engine.engineCommand()
// Register a callback that will be called when the block changes
watchCommand.createWatch((notification) => {
	console.log('Block changed', JSON.stringify(notification))
})

engine.run()
```

### Add a custom JavaScript block
```ts
import { initEngine, JsBlock } from 'logic-mesh';

// Initialize the engine
const engine = initEngine();

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
		variant: 'external',
		inputs: [{ name: 'in1', kind: 'Number' }, { name: 'in2', kind: 'Number' }],
		outputs: [{ name: 'out', kind: 'Number' }],
	} satisfies BlockDesc,
	// The block implementation
	function: async (inputs: unknown[]) => {
		return (inputs[0] as number) + (inputs[1] as number)
	},
} satisfies JsBlock

engine.registerBlock(JsAddBlock.desc, JsAddBlock.function)

engine.run()
```

