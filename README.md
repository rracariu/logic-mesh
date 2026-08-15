[![GitHub CI](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml/badge.svg)](https://github.com/rracariu/logic-mesh/actions/workflows/main.yml)
[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](https://github.com/rracariu/logic-mesh/blob/master/LICENSE)
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
| **Collections / Strings** | `Dict`, `List`, `Get`, `Keys`, `Values`, `Len`, `Concat`, `Replace` |

## Web editor & demos

A live SvelteKit editor is hosted at <https://rracariu.github.io/logic-mesh/>. Drag blocks, wire pins, watch the engine react.

It bundles a UI block set (`Slider`, `Gauge`, `Bar`, `Display`, `Led`, `Chart`, `MultiChart`, `Button`, `Checkbox`, `ComboBox`, `Table`, `Input`, `Label`) and five worked example programs you can switch between:

- **DAT Temperature Reset** — ASHRAE G36-style reset of supply-air SP from outdoor temperature, driving a PID loop.
- **Cooling Tower Stage + Lead/Lag** — demand → `Sequencer` → `LeadLag` → fan LEDs with on/off delays and rotation.
- **Air-Side Economizer (Enthalpy)** — `Enthalpy` of OA vs RA → `LessThan` → free-cooling LED, with both enthalpies on a `MultiChart`.
- **Anti-Short-Cycle Compressor** — `OnDelay` warmup + `OffDelay` cool-down lockout.
- **Outdoor Lighting (dusk-to-cutoff)** — `Sun` (sunrise/sunset) + `Schedule` + boolean composition driving a streetlight.

## Getting started

### Rust

```toml
[dependencies]
logic-mesh = "1.0"
```

Wire two sine waves into an adder and run them:

```rust
use logic_mesh::{
    base::{block::Block, block::connect::connect_output},
    blocks::{math::Add, misc::SineWave},
    SingleThreadedEngine,
};

#[tokio::main]
async fn main() {
    let mut add1 = Add::new();

    let mut sine1 = SineWave::new();
    sine1.amplitude.val = Some(3.into());
    sine1.freq.val = Some(200.into());
    connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("connected");

    let mut sine2 = SineWave::new();
    sine2.amplitude.val = Some(7.into());
    sine2.freq.val = Some(400.into());
    sine2.connect_output("out", add1.inputs_mut()[1]).expect("connected");

    let mut engine = SingleThreadedEngine::new();
    engine.schedule(add1);
    engine.schedule(sine1);
    engine.schedule(sine2);
    engine.run().await;
}
```

A multi-threaded engine is available behind the `multi-threaded` Cargo feature.

### Browser / Node.js

```sh
npm install logic-mesh
```

The npm package wraps the WASM build. Define a UI block in TypeScript:

```ts
import { defineBlock, initEngine } from 'logic-mesh';
import { z } from 'zod';

const Gauge = defineBlock({
  desc: { name: 'Gauge', dis: 'Gauge', lib: 'ui', ver: '0.0.1', category: 'UI', doc: 'Round gauge' },
  inputs: [['in', z.number()]] as const,
  outputs: [['out', z.number()]] as const,
  execute: async ([input]) => [input],
});

const engine = initEngine();
Gauge.register(engine);
// then wire blocks via engine.engineCommand() and engine.run()
```

## Possible applications

- **Building automation systems (BAS).** AHU/VAV/chiller sequences, schedules and overrides, energy logic, equipment runtime tracking. The block vocabulary maps directly to ASHRAE G36 patterns.
- **Edge / IoT controls.** Logic Mesh runs anywhere Rust runs, and reaches the browser through WASM for hand-held HMIs.
- **Reactive dashboards.** Use the engine as the live computation backbone behind charts, KPIs, or rule alerts.
- **Process simulations and digital twins.** Build models out of the same primitives used in production controls.
- **Custom low-code platforms.** The engine and the included editor are independent — keep the engine and ship your own UX.

## Architecture notes

- **Block trait** — every unit of work implements `async fn execute(&mut self)`. The `#[block]` attribute macro generates the boilerplate (description, registration, default impl).
- **Reactive scheduler** — blocks suspend on input pins via `read_inputs_until_ready` (event-driven) or `wait_on_inputs(timeout)` (event + periodic throttle). The engine only resumes blocks whose data has actually changed.
- **Type-checked pins** — pin kinds (`Number`, `Bool`, `Str`, `Dict`, `List`, `Null`) are validated at link time; mismatched values fault the receiving block instead of silently corrupting state.
- **Threading models** — single-threaded (default) and multi-threaded (`features = ["multi-threaded"]`) engines on native; WASM uses single-threaded with the browser event loop.
- **Auto-discovered registry** — `build.rs` walks `src/blocks/<category>/` and assembles the static block registry, so adding a new block is one file plus a `mod.rs` re-export.
- **Save/load format** — programs serialize to a stable JSON shape (blocks, links, positions, optional labels and per-program description) understood by both the Rust API and the web editor.

## Project layout

```
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
- **Stability:** APIs are stable enough for application use; expect occasional additive changes as new blocks land.
- **Issues and PRs welcome** — especially new blocks for under-served domains (lighting, irrigation, industrial process), additional UI primitives, and worked example programs.
