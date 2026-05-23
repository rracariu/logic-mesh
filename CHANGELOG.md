# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0]

First stable release. The Rust API surface is intentionally locked in here
so downstream code can depend on it without surprise breakage.

### Added

- Per-block actor execution model. Each scheduled block owns itself in a
  Tokio task; no `UnsafeCell`, no aliasing, no shared mutable state
  between blocks.
- Block-level fault propagation. Watch payloads carry a quality status
  (`Ok | Fault | Stale`), so a faulted upstream marks its downstream
  consumers and the UI renders both. Auto-recovery on the next clean
  execute.
- `MultiThreadedEngine` (`--features multi-threaded`). Block actor tasks
  are spawned via `tokio::spawn` directly onto the caller's tokio
  multi-thread runtime; the runtime's work-stealing scheduler handles
  them.
- Full save/load via the Rust API alone. New `Program` data type
  (blocks and links keyed by uuid, with per-block label, position, pin
  values, and `isConnected`); async `load_program` / `save_program` on
  both engines; new `LoadProgramReq` / `LoadProgramRes` engine
  messages.
- 80+ blocks in the standard catalog. New additions in this release:
  - Control: `Pid`, `Reset`, `Deadband`, `Clamp`, `Sequencer`,
    `LeadLag`, `TrimRespond` (ASHRAE G36 trim and respond),
    `Economizer`, `PriorityArray`.
  - Timers: `OnDelay`, `OffDelay`, `OneShot`, `RateLimit`, `Runtime`
    (accumulator), `CycleCount`.
  - Time: `Now`, `Schedule`, `Calendar`, `Sun` (sunrise/sunset by
    lat/lon).
  - Psychrometrics: `Enthalpy`, `Dewpoint`, `WetBulb` (Stull
    approximation).
  - Edge-detecting logic: `FlipFlop`, `Latch`, `Trigger`.
  - Stateful misc: `Ema`, `MovingAverage`, `Derivative`, `Integrator`,
    `ChangeOfValue`, `SampleHold`.
  - UI blocks (JS, via `defineBlock`): `Slider`, `Gauge`, `Bar`,
    `Display`, `Led`, `Chart`, `MultiChart`, `Button`, `Checkbox`,
    `ComboBox`, `Table`, `Input`, `Label`.
- SvelteKit web editor with five worked example programs: DAT
  Temperature Reset, Cooling Tower Stage + Lead/Lag, Air-Side
  Economizer (Enthalpy), Anti-Short-Cycle Compressor, Outdoor
  Lighting (dusk-to-cutoff).
- JS/TS block authoring via `defineBlock(...)` with Zod input/output
  schemas.
- Trait aliases `EngineBlock`, `MtBlock`, `BlockInput<R, W>`,
  `BlockOutput<W>` for terser bound spellings.
- ESLint + Prettier wired into the web workspaces and CI.

### Changed

- `BlockState` collapsed to `Running | Fault { reason } | Disabled |
  Terminated`. `Stopped` removed.
- `BlockProps` trait-object returns carry `+ Send` (required for the
  MT engine; transparent for all in-tree blocks). `BaseInput` requires
  `Writer: Send`; `BaseOutput` requires `L: Send`.
- `Block::execute` returns `impl Future<Output = ()> + Send` on
  native targets (cfg-gated; wasm32 keeps the non-Send form so
  `JsBlock` continues to work).
- `Input::try_take` now returns `Option<(Value, Status)>`.
  `Input::set_value` takes `(value, status)`. New `Input::status()`.
- New `Output::emit_status(Status)` for actor-driven fault marking.
- Engine to UI watch channel is unbounded (was bounded at 32). Fault
  notifications no longer drop under burst load.
- `Engine::load_blocks_and_links` replaced by
  `schedule_program_blocks` (sync, on the trait) plus `load_program`
  (async, inherent on each engine).
- `GetCurrentProgramRes` now returns `Result<Program, _>` rather than
  `Result<(Vec<BlockData>, Vec<LinkData>), _>`.
- `BlockInputData` (on the `inspect_block` snapshot) gained
  `is_connected: bool`.
- Module layout switched to post-2018 (`foo.rs` next to `foo/`).
  File paths changed; module paths did not.
- `read_block_inputs` drains every input that has a fresh value in
  one pass per cycle (temporally coherent snapshot per cycle).
- `read_inputs_until_ready` backs off exponentially up to 2 seconds
  when no inputs are reactive.
- `wait_on_inputs` returns early when an input arrives (no more
  double-sleep on every reaction).
- Watch channels use `send_if_modified`, so identical re-emissions do
  not wake downstream blocks. Convergent feedback loops quiesce.

### Removed

- `BlockState::Stopped` variant (operationally meaningless).
- `LinkState::Error` variant. Channel/transport failures surface as
  `Status::Stale` on the receiving input pin instead.
- `InputDefault` struct, the `BaseInput::default` field, the
  `InputProps::default()` trait method, and the
  `InputImpl::new_with_default` constructor. Nothing read them.

### Fixed

- `BlockHandle::desc` no longer holds a fabricated `&'static
  BlockDesc` that dangled for JS blocks once the block was moved into
  its actor task at schedule time. The handle now owns a cloned
  `BlockDesc`.
- `reset_connected_inputs` now filters by `data.is_connected`, so
  freshly-wired links no longer trigger wasted mailbox round-trips on
  unconnected inputs.
- UI: deleting a link now correctly notifies the engine. svelte-flow's
  built-in keyboard delete (Backspace/Delete) is bridged via the
  `ondelete` callback, so links no longer disappear visually while the
  source block keeps pushing values into the orphaned watch channel.
- UI: programs loaded from a saved file can now be deleted properly
  (the link UUID is read from `edge.id` for loaded programs,
  `edge.data.id` for in-session ones).

## Pre-1.0 history

For commits prior to 1.0, see the git log. Earlier 0.x releases were
not strictly versioned and several published 0.x crates have API
surfaces that 1.0 deliberately breaks (see the **Changed** /
**Removed** sections above).

[Unreleased]: https://github.com/rracariu/logic-mesh/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/rracariu/logic-mesh/releases/tag/v1.0.0
