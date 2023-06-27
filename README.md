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
use logic_mesh::{LogicMesh, LogicMeshBuilder, LogicMeshError, LogicMeshResult, LogicMeshValue, LogicMeshValueRef};
use std::sync::Arc;
use std::time::Duration;
```

