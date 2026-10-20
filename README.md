[![GitHub CI](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml/badge.svg)](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml)
[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](https://github.com/rracariu/logic-mesh/blob/main/LICENSE)
[![crates.io](https://img.shields.io/crates/v/logic-mesh.svg)](https://crates.io/crates/logic-mesh)
[![npm](https://img.shields.io/npm/v/logic-mesh.svg)](https://www.npmjs.com/package/logic-mesh)

# Logic Mesh

A reactive, async dataflow engine in Rust — wire blocks together, run programs natively or in the browser via WebAssembly, watch values propagate as inputs change.

![Example program](https://raw.githubusercontent.com/rracariu/logic-mesh/main/screen-shot.png)

## Why Logic Mesh

- **One engine, two targets.** The same Rust crate runs as a native library and as a `wasm32` build. The bundled web editor and a server-side controller speak to identical block semantics.
- **Built for control, not just dataflow.** First-class blocks for PID, setpoint reset, deadband, schedules, lead/lag rotation, equipment staging, runtime accumulation, on/off delays, EMA filtering, change-of-value gating, sunrise/sunset, psychrometrics — the vocabulary you reach for in HVAC, lighting, energy, and process control. ASHRAE Guideline 36 patterns map directly to the catalog.
- **Unit-aware numbers.** Inputs accept any compatible unit (`°F`, `°C`, `K`, `Pa`, `kPa`, `s`, `min`, `h`, …) and convert internally — courtesy of [libhaystack](https://crates.io/crates/libhaystack). Blocks like `Reset`, `Deadband`, `Clamp`, `EMA`, and `TrimRespond` propagate units to their outputs so downstream consumers see the right quantity.
- **Extensible from either side of the WASM boundary.** Define new blocks in Rust with the `#[block]` attribute macro, or in JavaScript/TypeScript with `defineBlock(...)` + Zod schemas when running in a browser.
- **Protocol-agnostic external integration.** A `Connector` trait (subscribe/publish/request) bridges the engine to any external system — MQTT, WebSockets, HTTP, or an in-process source. Generic `ExternalIn`/`ExternalOut`/`Request` blocks bind to a connector by name + address pins; concrete protocol implementations live outside the crate. Connectors can be implemented in JavaScript when running in a browser.
- **Async by construction.** Every block is a `Future`; the scheduler drives them on Tokio (or `wasm-bindgen-futures` in a browser) and only resumes blocks whose inputs have actually changed.

## Block catalog

| Category | Blocks |
| --- | --- |
| **Control** | `Pid`, `Reset`, `Deadband`, `Clamp`, `Sequencer`, `LeadLag`, `TrimRespond`, `Economizer`, `PriorityArray` |
| **Timers** | `OnDelay`, `OffDelay`, `OneShot`, `RateLimit`, `Runtime`, `CycleCount` |
| **Time** | `Now`, `Schedule`, `Calendar`, `Sun` |
| **Logic** | `And`, `Or`, `Not`, `Xor`, `Equal`, `NotEqual`, `GreaterThan`, `GreaterThanEq`, `LessThan`, `LessThanEq`, `FlipFlop`, `Latch`, `Trigger` |
| **Math** | `Add`, `Sub`, `Mul`, `Div`, `Modulus`, `Neg`, `Abs`, `Pow`, `Sqrt`, `Exp`, `Log10`, `LogN`, `Sin`/`Cos`/`Tan` (+ inverses), `Min`, `Max`, `Average`, `Median`, `Even`, `Odd` |
| **Misc** | `Ema`, `MovingAverage`, `Derivative`, `Integrator`, `ChangeOfValue`, `SampleHold`, `Random`, `SineWave`, `HasValue`, `ParseBool`, `ParseNumber` |
| **Bitwise** | `BitwiseAnd`, `BitwiseOr`, `BitwiseXor`, `BitwiseNot` |
| **Psychrometrics** | `Enthalpy`, `Dewpoint`, `WetBulb` |
| **External** | `ExternalIn`, `ExternalOut`, `Request` |
| **Collections / Strings** | `Dict`, `List`, `Get`, `Keys`, `Values`, `Len`, `Concat`, `Replace` |

## Web editor & demos

A live SvelteKit editor is hosted at <https://rracariu.github.io/logic-mesh/>. Drag blocks, wire pins, watch the engine react.

It bundles a UI widget set (`Slider`, `Gauge`, `Bar`, `Display`, `Led`, `Chart`, `MultiChart`, `Button`, `Checkbox`, `ComboBox`, `Table`, `Input`, `Label`) — each widget exchanges values with the engine through a JS-implemented `ui` connector and the generic `ExternalIn`/`ExternalOut` blocks — and five worked example programs you can switch between:

- **DAT Temperature Reset** — ASHRAE G36-style reset of supply-air SP from outdoor temperature, driving a PID loop.
- **Cooling Tower Stage + Lead/Lag** — demand → `Sequencer` → `LeadLag` → fan LEDs with on/off delays and rotation.
- **Air-Side Economizer (Enthalpy)** — `Enthalpy` of OA vs RA → `LessThan` → free-cooling LED, with both enthalpies on a `MultiChart`.
- **Anti-Short-Cycle Compressor** — `OnDelay` warmup + `OffDelay` cool-down lockout.
- **Outdoor Lighting (dusk-to-cutoff)** — `Sun` (sunrise/sunset) + `Schedule` + boolean composition driving a streetlight.

Widget configuration can also be driven by the running program: any config field (a slider's `max`, a bar's range, a LED label, …) may name the address of a plain `ExternalOut` block via the widget's `configSources` map, and the live value then overrides the literal default. Input widgets additionally accept a `valueSource` address whose published values they track as feedback — the classic HMI "setpoint follows the program until the operator overrides it" idiom — while user edits still push through the widget's own address. Drive an input widget's value through `valueSource`, not a `configSources` entry on its `value` key.

## Getting started

Logic Mesh 2.0 changed the API in places. Coming from 1.x, see the
[migration guide](https://github.com/rracariu/logic-mesh/blob/main/docs/migration-2.0.md); the
[CHANGELOG](https://github.com/rracariu/logic-mesh/blob/main/CHANGELOG.md) lists every change.

### Rust

The crate needs Rust 1.88 or newer. The engine runs on the application's
tokio runtime, so add tokio with the features the application uses:

```toml
[dependencies]
logic-mesh = "2.0"
tokio = { version = "1", features = ["rt", "macros", "time"] }
```

Wire two sine waves into an adder, run them, and read the sum through an
`EngineClient`, the typed handle that drives a running engine:

```rust,no_run
use std::time::Duration;

use logic_mesh::{
    base::block::connect_output,
    blocks::{math::Add, misc::SineWave},
    prelude::*,
    single_threaded::SingleThreadedEngine,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut add = Add::new();
    let sum = *add.id();

    let mut sine1 = SineWave::new();
    sine1.amplitude.set(3);
    sine1.freq.set(200);
    connect_output(&mut sine1.out, add.inputs_mut()[0])?;

    let mut sine2 = SineWave::new();
    sine2.amplitude.set(7);
    sine2.freq.set(400);
    sine2.connect_output("out", add.inputs_mut()[1])?;

    let mut engine = SingleThreadedEngine::new();
    engine.schedule(add)?;
    engine.schedule(sine1)?;
    engine.schedule(sine2)?;

    // A client is created while the engine is idle, and answered once it runs.
    let client = engine.client();
    let driver = async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let block = client.inspect_block(sum).await?;
        println!("sum = {:?}", block.outputs["out"].val);
        // `run()` resolves once the engine has shut down.
        client.shutdown().await
    };
    let (_engine, driven) = tokio::join!(engine.run(), driver);
    driven?;
    Ok(())
}
```

The single-threaded engine runs its blocks on the thread that drives it,
hence the current-thread runtime. A multi-threaded engine, which spreads the
blocks over a tokio multi-thread runtime, is behind the `multi-threaded`
Cargo feature; the application then enables tokio's `rt-multi-thread` itself
(see [`multi_threaded`](https://docs.rs/logic-mesh/latest/logic_mesh/multi_threaded/index.html#runtime)).

On `wasm32` the default `js` feature builds the JavaScript bindings the npm
package uses. A Rust application that builds the crate for `wasm32` with
bindings of its own turns it off with `default-features = false`.

### Browser / Node.js

```sh
npm install logic-mesh zod
```

The npm package wraps the WASM build, with TypeScript declarations. It is
ESM only, loads in Node.js `^22.19.0` or `>=24.5.0` and in bundlers set up
for WebAssembly, and takes [Zod](https://zod.dev) 4.6 or newer as a peer
dependency. The
[package README](https://github.com/rracariu/logic-mesh/blob/main/web/packages/logic-mesh/README.md) covers
the setup.

`startEngine()` creates the engine and starts it, and its `command` handle
sends requests to it. Define a custom block in TypeScript with
`defineBlock`, and register it before the engine starts:

```ts
import { createEngineSession, defineBlock } from 'logic-mesh';
import { z } from 'zod';

const Scale = defineBlock({
  desc: { name: 'Scale', dis: 'Scale', lib: 'custom', ver: '0.0.1', category: 'Math', doc: 'Multiplies by a factor' },
  inputs: [['in', z.number()], ['factor', z.number()]] as const,
  outputs: [['out', z.number()]] as const,
  execute: async ([input, factor]) => [input * factor],
});

// Blocks are registered on the engine before it starts.
const session = createEngineSession();
Scale.register(session.engine);
session.start();

const id = await session.command.addBlock('Scale', undefined, 'custom');
await session.command.writeBlockInput(id, 'factor', 2);
await session.command.writeBlockInput(id, 'in', 21); // out becomes 42
```

A failed command rejects with a `LogicMeshError` whose `code` names the
failure, such as `'BlockInstanceNotFound'`.

Or bridge the engine to an external system with a connector implemented in plain JavaScript:

```ts
import { registerConnector, startEngine } from 'logic-mesh';

registerConnector('sensors', {
  subscribe(address, callback) {
    const timer = setInterval(() => callback(readSensor(address)), 1000);
    return () => clearInterval(timer);
  },
  publish(address, value) { /* write toward the external system */ },
  request(address, value) { /* request/response */ },
});

// Creates the engine, prepares command handles, starts the message
// loop — the ordering-sensitive part is owned by the wrapper
const { command } = startEngine();

const id = await command.addBlock('ExternalIn');
await command.writeBlockInput(id, 'connector', 'sensors');
await command.writeBlockInput(id, 'address', 'zone-1/temp');
// `id`'s `out` pin now streams zone-1/temp values into the graph

await command.addConnector('sensors'); // attach to the running engine
```

For plain request/response functions there is a shortcut — `defineJsBlocks`
exposes JS functions (sync or async) as `Request` blocks through a single
generated connector:

```ts
import { defineJsBlocks, startEngine } from 'logic-mesh';

const session = startEngine();
const jsBlocks = defineJsBlocks({ scale: (value) => (value as number) * 2 });
await jsBlocks.attach(session.command);

const id = await jsBlocks.addBlock(session.command, 'scale');
await session.command.writeBlockInput(id, 'in', 21); // out becomes 42
```

The [connector guide](https://github.com/rracariu/logic-mesh/blob/main/docs/connectors.md) describes the
integration layer in depth: the `Connector` contract, the engine-managed
lifecycle, the external blocks, and their delivery and timeout guarantees.

## Possible applications

- **Building automation systems (BAS).** AHU/VAV/chiller sequences, schedules and overrides, energy logic, equipment runtime tracking. The block vocabulary maps directly to ASHRAE G36 patterns.
- **Edge / IoT controls.** Logic Mesh runs anywhere Rust runs, and reaches the browser through WASM for hand-held HMIs.
- **Reactive dashboards.** Use the engine as the live computation backbone behind charts, KPIs, or rule alerts.
- **Process simulations and digital twins.** Build models out of the same primitives used in production controls.
- **Custom low-code platforms.** The engine and the included editor are independent — keep the engine and ship your own UX.

## Architecture notes

- **Block trait** — every unit of work implements `async fn execute(&mut self)`. The `#[block]` attribute macro generates the boilerplate (description, registration, default impl).
- **Reactive scheduler** — blocks suspend on input pins via `read_inputs_until_ready` (event-driven) or `wait_on_inputs(timeout)` (event + periodic throttle). The engine only resumes blocks whose data has actually changed.
- **Typed pins** — every pin has a kind (`Number`, `Bool`, `Str`, `Dict`, `List`, `Null`, …). A value that reaches an input is converted to the input's kind, and one that cannot be converted faults the receiving block instead of silently corrupting state.
- **Fault isolation** — a faulted block marks its outputs, so the blocks downstream fault too, and it recovers on its own once a cycle completes cleanly. On native targets a panicking block faults instead of taking the engine down.
- **Typed requests and errors** — a running engine answers typed requests (`EngineRequest` / `EngineResponse`), correlated by id. `EngineClient` sends them as async methods, and every failure carries a stable `ErrorCode`, in Rust and as the `code` of the JavaScript `LogicMeshError`.
- **Connectors** — external systems implement the `Connector` trait (`subscribe`/`publish`/`request`, plus optional `start`/`stop`); the engine manages their lifecycle, and connectors can be attached to or detached from a running engine through `EngineClient` requests. `ExternalIn` streams subscribed values into the graph, `ExternalOut` publishes wired values, and `Request` does request/response — all with a timeout whose deadline survives cancellation. See `examples/connector_demo.rs` for a concrete implementation and the [connector guide](https://github.com/rracariu/logic-mesh/blob/main/docs/connectors.md) for the design.
- **Threading models** — single-threaded (default) and multi-threaded (`features = ["multi-threaded"]`) engines on native, configured through the same `Engine` trait; WASM uses single-threaded with the browser event loop.
- **Auto-discovered registry** — `build.rs` walks `src/blocks/<category>/` and assembles the static block registry, so adding a new block is one file plus a `mod.rs` re-export.
- **Save/load format** — programs serialize to a versioned JSON format understood by both the Rust API and the web editor: blocks with their type version, pin values, labels, positions and opaque editor metadata; links with their ids; the program's name and description. Loads are all or nothing, saves from 1.x load unchanged, and a program from a newer format is rejected by its version.

## Project layout

```text
src/
  base/          core traits, engine, link/pin model
  blocks/        block implementations, organized by category
  tokio_impl/    native (Tokio) reader/output/engine impls
  wasm/          wasm-bindgen entry points + JS-facing types
block_macro/     #[block] proc-macro
web/
  packages/logic-mesh/   TypeScript wrapper around the WASM build
  app/                   SvelteKit web editor (the demo at the link above)
```

## Status & contributing

- **License:** BSD-3-Clause.
- **Stability:** the crate and the npm package follow [semantic versioning](https://semver.org/spec/v2.0.0.html). 2.x releases are additive: new blocks, new APIs, and new variants of the `#[non_exhaustive]` enums and error codes. Breaking changes wait for 3.0. Every 2.x release loads the programs that 1.x and earlier 2.x releases saved. Error codes are stable; error messages are meant for people and may change. Items marked `#[doc(hidden)]`, such as the block macros' support module, are not public API.
- **MSRV:** Rust 1.88. The minimum supported Rust version may rise in a minor release, never in a patch release, and the CHANGELOG says so when it does.
- **JavaScript runtimes:** Node.js `^22.19.0` or `>=24.5.0`, and browsers through a bundler with WebAssembly ESM support (Vite with `vite-plugin-wasm`, webpack 5 with `asyncWebAssembly`).
- **Issues and PRs welcome** — especially new blocks for under-served domains (lighting, irrigation, industrial process), additional UI primitives, and worked example programs.
