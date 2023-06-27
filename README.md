[![GitHub CI](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml/badge.svg)](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml)
[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](https://github.com/rracariu/logic-mesh/blob/master/LICENSE)

# Logic Mesh
A logic engine that is fully async, dynamic, and reactive written in Rust.

## Possible applications
Logic Mesh started as a hardware control engine, but the scope expanded to anything that requires a reactive and dynamic evaluation engine. 
It is designed to be used in a variety of applications, from hardware controls, games to web servers. It is designed to be fast, efficient, and easy to use.

The included WASM support allows it to be used in web applications, with the option to use the same codebase for both the frontend and backend.

There is a sample low code editor that is built on top of Logic Mesh, which can be found [here](link). It serves as an example of how Logic Mesh can be used, and as
a simple way to experiment with the Logic Mesh engine.

## Features
- Fully async and reactive
- WASM support
- Uses Tokio for async runtime
- The API is simple enough to be used in a variety of applications
- A low code editor is included as an example of how Logic Mesh can be used
- A growing library of built-in blocks
- Extensible with custom blocks, either in Rust or JavaScript when running in a WASM environment

## Examples

The following examples are written in Rust.

```rust
// Init a Add block
let mut add1 = Add::new();
let add_uuid = *add1.id();

// Init a SineWave block
let mut sine1 = SineWave::new();

// Se the amplitude and frequency of the sine wave
sine1.amplitude.val = Some(3.into());
sine1.freq.val = Some(200.into());
// Connect the output of the sine wave to the first input of the add block
connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

// Init another SineWave block
let mut sine2 = SineWave::new();
sine2.amplitude.val = Some(7.into());
sine2.freq.val = Some(400.into());

// Connect the output of the sine wave to the second input of the add block
sine2
	.connect_output("out", add1.inputs_mut()[1])
	.expect("Connected");

// Init a single threaded engine
let mut eng = SingleThreadedEngine::new();

// Schedule the blocks to be run
eng.schedule(add1);
eng.schedule(sine1);
eng.schedule(sine2);

// Run the engine
eng.run().await;
```

