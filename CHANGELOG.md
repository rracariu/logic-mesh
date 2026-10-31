# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0] - Unreleased

Settles the API for the 2.x series: typed errors with stable codes, a
typed request/response protocol driven through `EngineClient`, a
versioned save format, one canonical path per item, and JavaScript
bindings with typed declarations and coded errors. It also fixes how the
engines start, stop, pause, fail, save and load. From 2.0 on, 2.x
releases are additive and breaking changes wait for 3.0.

Thank you to Souyama ([@sansmoraxz](https://github.com/sansmoraxz)), whose
four pull requests are part of this release: the connector API and the
external blocks the web editor's widgets run on ([#13]), block macros that
work outside this crate ([#5]), the JS tests running in CI ([#7]), and the
doc comment conventions that the `missing_docs` lint now enforces ([#12]).

### Breaking changes

The [migration guide](docs/migration-2.0.md) walks through each of these
with before and after code, for Rust and for JavaScript and TypeScript.

- **Install:** Rust 1.88 or newer. logic-mesh no longer enables tokio's
  `rt-multi-thread`, so applications enable the runtime features they use
  on their own tokio. On wasm32 the JavaScript bindings are behind the
  default `js` feature. The npm package takes zod `^4.6.0` as a peer
  dependency and declares Node.js `^22.19.0 || >=24.5.0`.
- **Paths:** every public item has one path. Errors are at the crate root,
  built-in blocks at their category (`blocks::math::Add`), and the base
  types at their parent module. A new `logic_mesh::prelude` gathers the
  traits and macros.
- **Engines:** `Engine::schedule` returns a `Result` and rejects duplicate
  ids. `run(self)` hands the idle engine back. The scan interval is per
  engine (`set_sleep_dur` is gone), and the `Engine` trait is sealed.
- **Protocol:** `EngineMessage` is replaced by `EngineRequest` and
  `EngineResponse`, correlated by request id and driven through
  `EngineClient`. Every request is answered, control requests included,
  with a typed `RequestError` carrying an `ErrorCode`. A `WatchMessage`
  lists its changes in a `Vec`.
- **Errors:** `anyhow` is replaced by typed errors (`logic_mesh::Error` and
  one enum per subsystem), all `#[non_exhaustive]`.
- **Blocks:** the block macros report misuse as compile errors, `BlockDesc`
  and the pin descriptors are built with builders, `Block` needs only
  `BlockProps`, connect functions return `ConnectError`, the pin and
  link fields are private behind accessors, `InputProps::get_value` is
  renamed `value`, `BlockProps::state()` returns a reference, and
  `Input::receiver()` resolves with a `bool`.
- **Save format:** programs carry a format `version` and each block its
  type's `ver` and opaque `editor` metadata, `positions` is renamed
  `position`, `ProgramBlock::lib` is renamed `library` (still saved as
  `"lib"`), the program types are `#[non_exhaustive]`, and loads are all or
  nothing. 1.x saves load unchanged.
- **JavaScript:** commands reject with coded `LogicMeshError`s, watches can
  be closed, every value handed to JavaScript is JSON-compatible (plain
  objects, `null` for a missing value), the scan interval is a `number` of
  milliseconds (`scanInterval`), and the declarations are typed.

### Added

#### Engine

- Engine phases in the type: `SingleThreadedEngine<Idle>` and
  `MultiThreadedEngine<Idle>` are configured, and `run()` drives them in
  the `Running` phase, which configuration code cannot reach ([#18]).
- `Engine` trait methods implemented by both engines: `add_connector`,
  `managed_connectors` (named in [#29]), `block_handles`, `block_handle`,
  `set_block_metadata` ([#27]), `set_block_editor` ([#28]), `client` ([#29]), the
  per-engine scan interval `scan_interval`, `set_scan_interval` and
  `with_scan_interval` ([#27]), and the associated type `RunFuture`, so
  generic code can require a `Send` run future ([#27]).
- `SingleThreadedEngine::schedule_local` hosts blocks that are not
  `Send + Sync` ([#18]).
- Block panics are isolated on native targets: a panicking block enters
  `BlockState::Fault` with reason `block panicked: <message>`, faults its
  outputs and so the blocks downstream, stays inspectable, savable and
  removable, and is retried after 1 s, doubling up to 60 s, until a cycle
  completes without panicking ([#22]).
- One documented lifecycle contract for `Shutdown`, `Pause`, `Resume` and
  `Reset`, shared by both engines ([#22]).
- `EngineError` variants `DuplicateBlockId` ([#22]), `NoRuntime` ([#27]),
  `InputNotFound` ([#18]), `OutputNotFound`, `LinkBlockNotInProgram`,
  `DuplicateLink`, `DuplicateLinkId`, `InvalidLinkUuid` ([#23]),
  `UnsupportedProgramVersion`, `BlockVersionMismatch` ([#28]),
  `ConnectorAlreadyManaged`, `ConnectorNotManaged`, `ConnectorStartFailed`,
  `ConnectorStartTimeout`, `EngineGone`, `UnexpectedResponse`,
  `EvalPanicked` and `EvalTimeout` ([#29]).
- `Debug` for both engines, `BlockHandle` and `ConnectorHandle` ([#26]).

#### External integration

- The `Connector` trait (`subscribe`, `publish`, `request`, optional `start`
  and `stop`), a process-wide connector registry, and `ConnectorHandle`
  ([#13]).
- The generic `ExternalIn`, `ExternalOut` and `Request` blocks, which bind
  to a connector by name and address pins ([#13]). All three have a
  `timeout` pin (raw milliseconds, default 5000), whose deadline counts
  from the first issue of an operation and survives cancelled cycles
  ([#23]).
- Engine-managed connector lifecycle: a run starts the managed connectors
  before any block executes, `Shutdown` stops them, `Reset` unregisters and
  stops them, and connectors can be attached to and detached from a
  running engine ([#13]).
- `ConnectorError`, with constructors and an optional source attached with
  `with_source` ([#13], [#26]).
- `examples/connector_demo.rs` and the [connector guide](docs/connectors.md)
  ([#13]).

#### Blocks and the block macros

- The block macros work outside this crate: generated code uses
  `::logic_mesh` paths, `#[logic_mesh(crate = "path")]` names a renamed
  dependency, and `blocks::InputImpl` and `blocks::OutputImpl` are public
  ([#5]).
- `BlockDesc::new(name, library)` and its `with_*` builders,
  `BlockPin::new` and `OutDesc::new` ([#24]).
- The `#[ver = "..."]` attribute is registered ([#24]).
- `InputImpl::set(value)`, `BaseInput::connection_count()`,
  `BaseOutput::effective_status()`, `BaseLink::connected(..)` and
  `BaseLink::tx()` ([#26]).
- `ConnectError`, the typed error of the connect and disconnect functions
  ([#24]), and `BlockDescError`, the typed error of block descriptions ([#26]).
- `RegistryError::BlockAlreadyRegistered` for a block type registered
  twice ([#5]).

#### API surface

- `logic_mesh::prelude` ([#27]).
- Re-exports at the crate root: `Value`, `HaystackKind` and `Uuid` ([#5]),
  `libhaystack` ([#27]), `serde_json` ([#28]), and the tokio channel
  modules as `logic_mesh::sync::{mpsc, watch}` ([#27]).
- Typed errors: `Error`, `Result`, `RegistryError`, `EngineError`,
  `ValueError` and `ExternalError` ([#11]), joined by `ConnectError` ([#24]),
  `ConnectorError`, `ConnectorErrorSource`, `BlockDescError` ([#26]) and
  `RequestError` ([#29]). `Error` wraps each subsystem error, so `?`
  converts any of them ([#11], [#27]).
- `ErrorCode`, a stable, serializable name for every failure, and `code()`
  on every error type. A code from a newer release reads as
  `ErrorCode::Unknown` ([#29], [#30]).
- Constructors for the structs the engines produce: `WatchMessage::new`,
  `BlockDefinition::new` with `with_input`, `with_output` and `with_state`,
  `BlockInputData::new` and `BlockOutputData::new` ([#26]).

#### Save format

- `ProgramBuilder`, which checks each block, pin value and link against the
  registry as it is added ([#18]).
- `Program::version` and `Program::FORMAT_VERSION` (1), and
  `Program::check_version` ([#28]).
- `ProgramBlock::ver`, the block type's version, checked on load ([#28]).
- `ProgramBlock::editor`, opaque editor metadata (`EditorMetadata`, a
  `serde_json` object) saved back unchanged, with `BlockHandle::editor`,
  `ProgramBlockBuilder::editor` and `Engine::set_block_editor` ([#28]).
- Constructors for `Program`, `ProgramBlock`, `LinkData` (with `with_id`),
  `PinValue` and `Position`, and `PartialEq` on the program types ([#28]).
- The format versions are documented in the `base::program` module ([#28]).

#### Engine protocol

- `EngineRequest` and `EngineResponse`, with named fields; `Envelope`,
  `ClientId`, `RequestId` and `WatchId`; and the aliases `RequestMessage`,
  `ResponseMessage`, `RequestSender`, `ResponseSender` and `WatchSender`
  ([#29]).
- `EngineClient`: an async method per request, `Clone`, and `Send + Sync`
  on native targets, created with `Engine::client`. `watch()` returns a
  `Watch` with its id and notification receiver ([#29]).
- The `SetBlockMetadata` and `SetBlockEditor` requests ([#29]).

#### JavaScript and TypeScript

- `registerConnector`, `unregisterConnector`, `connectorRegistered` and
  `connectorIs` for connectors implemented in JavaScript, and the
  `addConnector`, `removeConnector` and `listConnectors` commands ([#13]).
- `startEngine`, `createEngineSession` and `EngineSession`, which own the
  engine start sequence, and `defineJsBlocks`, which exposes JS functions
  as `Request` blocks ([#13]).
- `LogicMeshError`, `LogicMeshErrorCode` and `isLogicMeshError` ([#29]).
- `removeWatch`, `setBlockMetadata` and `setBlockEditor` commands ([#29]).
- `setLogLevel` and the `LogLevel` type ([#30]).
- `WatchSubscription`, which `EngineSession.watch` resolves with, and
  `EngineSession` implementing `AsyncDisposable` ([#30]).
- `EngineSession.unwatch` ([#29]).
- Exported types `Program`, `ProgramBlock`, `PinValue`, `Position`,
  `EditorMetadata` and `PROGRAM_FORMAT_VERSION` ([#28]), and `JsBlockExecutor`,
  `JsBlockFactory`, `BlockInspection` and `BlockState` ([#30]).
- `ProgramSchema` and `programSchema(editor)`, Zod schemas of the save
  format that check a program without the engine, each problem at its
  path, and the editor metadata with a schema of the caller's.
  `z.toJSONSchema(ProgramSchema, { io: 'input' })` gives the format as a
  JSON Schema ([#35]).
- The cargo feature `js`, on by default, gates the JavaScript bindings on
  wasm32 ([#30]).

#### Web editor

- Its UI widgets exchange values with the engine through a JS-implemented
  `ui` connector and the generic external blocks. A widget's config can
  follow a live value (`configSources`), and an input widget can track a
  feedback address (`valueSource`) ([#13]).
- The editor saves the versioned format and upgrades older saves on load
  ([#28]).
- Each widget lists the sources it reads without a link (its
  `valueSource`, `configSources` and MultiChart series addresses), and
  hovering one outlines the block that publishes it ([#34]).

#### Tooling and release

- CI checks, lints and documents every workspace member, the crate on its
  default features alone, wasm32 with and without default features, and the
  MSRV, plus Package and Version sync jobs ([#11], [#20], [#27], [#30]).
- The JS tests run in CI ([#7]), and so do the web app's tests ([#28]),
  before every npm release too ([#33]).
- Compile-fail tests for the block macros (`tests_lib_usage`, with
  trybuild) and tests of the crate used from outside, renamed or not ([#5],
  [#24]).
- docs.rs documents the `multi-threaded` feature ([#20]) and the wasm32
  target ([#30]).
- Every public item has a doc comment, enforced by the workspace-wide
  `missing_docs` lint ([#12]).
- The examples of the migration and connector guides compile and run as
  doctests.
- The npm package checks: `check:package` (publint and attw) and
  `test:pack`, which installs the packed tarball and runs it on plain Node
  ([#21]).

### Changed

#### Engine

- `Engine::schedule` returns `logic_mesh::Result<()>`, and on native
  targets requires the block to be `Send + Sync` ([#18]). It, the registry's
  scheduling functions and `AddBlock` with a given id reject an id that is
  already scheduled with `EngineError::DuplicateBlockId`, instead of
  replacing the block ([#22]).
- `Engine::run` takes `self`, returns `Self::RunFuture`, and resolves to the
  idle engine after `Shutdown` ([#18], [#27]).
- The `Engine` trait is sealed: only this crate's engines implement it
  ([#27]).
- The scan interval is per engine, set with `Engine::set_scan_interval` or
  `with_scan_interval`, instead of the process-wide `set_sleep_dur` ([#27]).
- `block_handles`, `block_handle` and the single-threaded engine's
  `set_block_metadata` are `Engine` trait methods instead of inherent ones,
  so calling them needs the trait in scope (`logic_mesh::prelude::*` or
  `base::engine::Engine`) ([#27]).
- `Engine::set_block_metadata` returns a `Result` and rejects an unknown id
  with `EngineError::BlockInstanceNotFound` ([#27]).
- `MultiThreadedEngine`: `schedule_send` is replaced by `schedule` ([#18]);
  scheduling outside a tokio runtime returns `EngineError::NoRuntime`
  instead of panicking ([#27]); `Shutdown` keeps every block, link and pin
  value instead of terminating them, so watchers no longer see a
  `Terminated` state for each block ([#22]); blocks execute only inside
  `run()` ([#22]).
- Both engines execute blocks only while running and not paused, and a run
  first starts the managed connectors and wires the links queued while
  idle. While paused, no block executes and every request is answered
  ([#22]).
- Only a completed `execute` cycle clears a block's fault. A request, pause
  or shutdown that cuts a cycle short leaves it in `Fault` ([#22]).
- A pin write through the engine runs the block's next cycle, linked or
  not, and the value is kind-converted like a linked one, so a value of the
  wrong kind faults the block. `InputReader::read_inputs`,
  `read_inputs_until_ready` and `wait_on_inputs` drain every input that has
  a value ([#23]).
- Program loads are all or nothing. `schedule_program_blocks` and
  `LoadProgram` check the whole program before scheduling any block. A
  wiring failure in `LoadProgram` removes the program's blocks again; a link
  or pin value of a `schedule_program_blocks` program that still fails when
  `run()` starts is dropped with a warning. A program's links must
  join its own blocks (`LinkBlockNotInProgram`), its saved values must name
  existing pins (`InputNotFound`, `OutputNotFound`), its link ids must be
  free UUIDs (`InvalidLinkUuid`, `DuplicateLinkId`), and no two links may
  join the same pins (`DuplicateLink`) ([#22], [#23]).
- `schedule_program_blocks` applies the program's saved pin values when
  `run()` starts ([#23]).
- Saving a program returns the loaded program's name and description, and
  keeps link ids. A block whose actor is gone is saved without its pin
  values instead of failing the save, and is removed with its links ([#23]).
- A `Connect` request keeps the link id it is given, and rejects a taken
  or non-UUID id (`DuplicateLinkId`, `InvalidLinkUuid`) and a missing pin
  (`PinNotFound`) before wiring anything ([#23]).
- Removing a link looks it up in the engine's link index, so a link a block
  adds to itself inside `execute()` cannot be removed by id ([#23]).
- `single_threaded::BlockHandle` and `multi_threaded::BlockHandle` became
  one struct, `base::engine::BlockHandle` ([#23], [#27]).
- logic-mesh enables tokio's `rt`, `sync`, `time` and `macros` only, with or
  without `multi-threaded`. An application that builds a multi-thread
  runtime enables `rt-multi-thread` on its own tokio ([#27]).

#### Blocks and the block macros

- The macros report misuse as spanned compile errors instead of panicking or
  silently ignoring it: arguments to `#[block]`, a missing category or
  output, unknown, duplicate or non-string attributes and pin arguments, a
  pin attribute on a field of another type, a struct-level `#[output]`, a
  block-level `#[input]` without an integer `count`, and duplicate pin
  names ([#24]).
- `#[block]` takes no arguments ([#24]).
- `desc.doc` joins a doc comment's lines with `\n` and keeps blank lines
  ([#24]).
- A bare `#[input]` or `#[output]` declares a pin of kind `Null` named after
  its field, and a raw field name (`r#in`) names the pin without the prefix
  ([#24]).
- `logic_mesh_block_macro` is pinned to `=2.0.0`, and its generated code
  calls the hidden `logic_mesh::__private` module ([#24]).
- `Block` requires `BlockProps` only, and `BlockConnect` has a blanket impl
  for every `BlockProps` type ([#24]).
- `BlockStaticDesc::desc()` is renamed `static_desc()` ([#24]).
- `BlockProps::state()` returns `&BlockState` instead of a clone, and
  `BlockProps::set_state` returns nothing instead of a clone of the new
  state ([#36]).
- `Input::receiver()` resolves with a `bool`, whether a fresh payload
  arrived, instead of a clone of the value, which `try_take` still returns
  ([#36]).
- The connect and disconnect functions and `BlockConnect` methods return
  `ConnectError` instead of `&'static str` ([#24]).
- `BlockDesc`, `BlockPin`, `OutDesc`, `BlockImplementation` and
  `BlockRunCondition` are `#[non_exhaustive]` ([#24]).
- `BlockImplementation` and `BlockRunCondition` parse with `FromStr`
  (`BlockDescError`) instead of `TryFrom<&str>` ([#26]).
- The fields of `BaseInput`, `BaseOutput` and `BaseLink` are private to the
  crate ([#26]).
- `InputProps::get_value` is renamed `value`, the name `OutputProps::value`
  has ([#27]).
- `schedule_block`, `schedule_block_with_uuid` and `eval_static_block` take
  the block's library ([#5]).
- A lookup without a library searches every library, everywhere: the
  registry functions, the engine requests and the JS commands. A name that
  more than one library has is `AmbiguousBlockName` ([#27]).
- `get_block`, `get_core_block` and `make` return
  `Result<_, RegistryError>` instead of `Option` ([#27]), and `register` and
  `register_block_desc` return `Result<(), RegistryError>` ([#5], [#11]).
- `BlockEntry::make` is a method ([#18]).

#### API surface

- Every public item has one canonical path: errors at the crate root,
  blocks at `blocks::<category>::<Block>`, the base types at their parent
  module (`base::block`, `base::input`, `base::output`, `base::link`,
  `base::program`), `Status` at `base::Status`, and `BlockHandle` in
  `base::engine` ([#27]).
- `anyhow` is replaced by typed errors ([#11]).
- libhaystack is upgraded to 4.0, a public dependency ([#17]).
- `serde_json` is a normal, public dependency ([#28]).
- `Status`, `BlockState`, `LinkState` and `ChangeSource` are
  `#[non_exhaustive]`, and so are `WatchMessage`, `BlockDefinition`,
  `BlockInputData` and `BlockOutputData` ([#26]).

#### Save format

- `Program` starts with a `version`. A save without one (1.x) reads as
  version 0 and loads unchanged. A newer version fails to deserialize
  before its blocks and links are read, and an engine rejects it with
  `UnsupportedProgramVersion` ([#28]).
- A block whose saved `ver` differs from the registered block's in major
  version (or in minor version below 1.0) fails the load with
  `BlockVersionMismatch` ([#28]).
- `ProgramBlock::positions` is renamed `position`, serialized as
  `"position"`; `"positions"` is still read ([#28]).
- `ProgramBlock::lib` is renamed `library`, and is still serialized as
  `"lib"` ([#28]).
- `LinkData` without an id no longer writes `"id": null` ([#28]).
- `Program`, `ProgramBlock`, `LinkData`, `PinValue` and `Position` are
  `#[non_exhaustive]` ([#28]).

#### Engine protocol

- `EngineMessage` is replaced by `EngineRequest` and `EngineResponse`.
  Responses are correlated by `RequestId`, so any number of requests can be
  in flight on a channel. Every request is answered once it took effect,
  `Pause`, `Resume`, `Reset` and `Shutdown` included ([#29]).
- Responses carry `RequestError` (code, message with the source chain, and
  the typed error within the process) instead of strings ([#29]).
- `Engine::create_message_channel(responses)` returns the engine-assigned
  `ClientId` and the request sender. The `Engine::Channel` associated type
  and the `Messages` alias are removed ([#29]).
- A watch is identified by an engine-assigned `WatchId`, and a client can
  hold any number of them ([#29]).
- `WatchMessage::changes` is a `Vec<ChangeSource>`, outputs first and each
  in the block's pin order, instead of a `HashMap` keyed by pin name
  ([#36]).
- The multi-threaded engine's public async methods are reached through
  `EngineClient` instead ([#18], [#29]).
- `BlockDefinition`, the answer to `InspectBlock`, has `id: Uuid` and
  `state: BlockState`, which carries the fault reason, instead of string
  fields and `fault_reason`. It serializes camelCase, `faultReason`
  included ([#29]).

#### JavaScript and TypeScript

- `EngineCommand` methods take overlapping calls on one handle ([#29]).
- Commands reject with `LogicMeshError`s carrying a `code` instead of
  strings. `registerBlock`, `registerConnector`, `initEngine`, the
  `BlocksEngine` constructor and `setLogLevel` throw them too ([#29], [#30]).
- An unparseable block or link id rejects with `InvalidBlockUuid` or
  `InvalidLinkUuid` instead of becoming the nil UUID ([#29]). A value that is
  not a Haystack value rejects with `InvalidValue` instead of being written
  as Null or as a Dict of its properties ([#30]).
- `loadProgram` rejects a newer format with `UnsupportedProgramVersion` and
  any other unreadable payload with `InvalidProgram` ([#28], [#29]).
- Every value handed to JavaScript is JSON-compatible: a Dict is a plain
  object and Null or a missing value is `null`, instead of a `Map` and
  `undefined`. That covers `getProgram` ([#28]), `inspectBlock`,
  `createLink`, the pin writes, `evalBlock`, watch notifications, JS block
  inputs and JS connector values ([#30]). `inspectBlock` returns a
  `BlockInspection`, keyed `lib` and `faultReason` ([#30]).
- `registerBlock` throws `InvalidArgument` for a factory that is not a
  function, `null` or `undefined` ([#30]).
- A faulting block logs at debug instead of at error on every cycle, and
  `Sub` no longer logs its fault. A panicking block still logs at error, once
  per retry delay ([#30]).
- `createWatch` resolves with the watch id once the watch is open ([#29]).
- `pauseExecution`, `resumeExecution`, `resetEngine` and `stopEngine`
  resolve once the engine has applied them ([#29]). A paused engine answers
  every command ([#22]).
- `new BlocksEngine(scanInterval)` and `initEngine(scanInterval)` take a
  validated `number` of milliseconds, set the interval of the engine they
  create only, and throw `InvalidArgument` for a negative, non-finite or
  too large value ([#27], [#30]).
- The generated declarations name the package's types instead of `any`, and
  `LinkData` is a `type` ([#30]).
- The `Program` types follow the save format: `version`, `ver`, an optional
  `position` (was `positions`), `editor` (the block-level `widget` is
  removed), and `PinValue` with both fields required. `Program<Editor>` and
  `ProgramBlock<Editor>` take the editor metadata type ([#28]).
- The module start hook runs once, keeps an installed logger, and logs at
  warn by default instead of trace ([#30]).
- A JS block executor may return its outputs directly, and a throw or
  rejection faults the block with `JS execute failed: <message>` ([#30]).
- `defineBlock` maps nullable, optional, default, literal and single-kind
  union schemas to pin kinds, and copies the description it is given ([#30]).
- Block descriptions can have multi-line `doc` strings ([#24]).
- Rust side of the bindings (wasm32): `BlocksEngine::new` and `init_engine`
  take `Option<f64>` and return `Result<BlocksEngine, JsValue>`;
  `BlocksEngine::register_block`, `register_connector` and
  `register_js_connector` return `JsValue` errors ([#30]).

#### npm package

- zod is a peer dependency, `^4.6.0`, instead of a regular dependency
  ([#21]).
- `engines.node` is `^22.19.0 || >=24.5.0` ([#21]).
- `exports` adds the `default` condition and `./package.json`, and `main`
  serves tools that predate `exports` ([#21]).
- `sideEffects` lists only the wasm glue, so a bundle that imports only
  `defineBlock` or types leaves the engine out ([#21]).
- The tarball ships the LICENSE ([#21]).
- The package metadata names its author, homepage, issue tracker and
  repository directory ([#21]).

#### Web editor

- `Gauge` is display-only, and `Input` and `ComboBox` lost their `in` pins
  in favour of `valueSource`. Programs saved before are migrated on load
  ([#13]).

#### Tooling and release

- The crates.io release stops if the macro publish fails, checks that the
  release tag matches the crate and npm versions, publishes with
  `--locked`, and skips versions already published. The npm release runs
  the checks first and publishes prereleases to the `next` dist-tag ([#20]),
  and runs every check a pull request gets, on the build it publishes
  ([#32]).
- The macro crate ships the LICENSE too, and the Package job checks that
  both `.crate` files carry it ([#32]).
- `rust-version = "1.88"` is declared ([#20]).
- The npm build cleans its output first, and `prepublishOnly` refuses an
  incomplete or out-of-date build ([#21]).

### Removed

- `MultiThreadedEngine::add_block(name, id, lib)`: schedule a registered
  block with `blocks::registry::schedule_block` or
  `schedule_block_with_uuid` while the engine is idle, or with
  `EngineClient::add_block` while it runs ([#18]).
- `schedule_block_send` and `schedule_block_send_with_uuid` ([#18]).
- `blocks::utils::set_sleep_dur` and `get_sleep_dur` ([#27]).
- `disconnect_block`, `disconnect_link`, `ProgramMeta`, and the public
  `register_blocks!`, `eval_block_impl` and `BlockData` ([#27]).
- The duplicate paths to every public item, including `base::status`, the
  blocks' leaf modules and `multi_threaded::engine` ([#27]).
- `TryFrom<&str>` for `BlockImplementation` and `BlockRunCondition` ([#26]).
- `EngineMessage`, `Messages` and `Engine::Channel` ([#29]).
- The public `wasm::types` and `wasm::js_block` modules (wasm32), with
  `JsBlockDesc`, `JsBlockPin`, `JsWatchNotification`, `JsWatchChange` and
  `JsBlock`: they only serialize the bindings' values ([#30]).

### Fixed

- A block actor whose mailbox closed spun forever instead of ending ([#22]).
- Scheduling an id twice orphaned the first block, whose actor kept running
  ([#22]).
- A stray response message sent to an engine panicked its event loop, and
  on wasm the whole instance ([#22]).
- Reset could deadlock the single-threaded engine on a full mailbox ([#22]).
- Dropping a multi-threaded engine left its actors running ([#22]).
- A failed load left its validated links queued for the next one ([#18]).
- A load that failed partway left the blocks scheduled before the failure
  ([#22], [#23]).
- A pin write to an unlinked input did not run the block ([#23]).
- Steady engine requests could keep a slow connector operation from ever
  timing out, and `ExternalIn`'s subscribe had no timeout ([#23]).
- Watchers whose receiver was gone were never dropped ([#23]).
- A save lost the program's name, description and link ids, and failed
  whole if one block's actor was gone ([#23]).
- `BitwiseNot` wrote its result without propagating it to linked blocks
  ([#26]).
- A block that panicked while evaluated ended the engine's run, and one
  waiting on an input the evaluation did not supply left the engine
  answering nothing. Such an evaluation now fails with `EvalPanicked`, or
  with `EvalTimeout` after 5 s ([#29]).
- On wasm32, a block fed by its own output, directly or through other
  blocks, froze the page or Node process ([#30]).
- A JS block registered without a factory was rejected with
  `ExternalLibraryNotFound` unless another block of its library had one
  ([#30]).
- The wasm start hook panicked when a logger was already installed ([#30]).
- A JS block that returned its outputs synchronously threw through the wasm
  frames, and a thrown `Error` faulted the block with a serde message ([#30]).
- Scan intervals of 2^31 ms or more wrapped in `setTimeout` on wasm32 ([#30]).
- `defineBlock` wrote the pins into the caller's description, so two blocks
  defined from one description shared the last one's pins ([#30]).
- The npm package's `require.resolve('logic-mesh')` failed with
  `ERR_PACKAGE_PATH_NOT_EXPORTED`, and stale build output could ship ([#21]).
- The block macro was unusable outside this crate ([#5]).
- `Logn` (LogNatural) computed the base-2 logarithm instead of the natural
  one ([#36]).
- `Dict` panicked in a debug build when its `values` list was empty
  ([#36]).
- `Median` panicked when none of its inputs held a number, as when linked
  from an output that was never set, which on wasm32 aborted the engine
  ([#36]).
- When an input and an output shared a name, which a JS block or a
  hand-written `BlockProps` can have, watch notifications carried only one
  of them ([#36]).

### Changes to APIs added after 1.0.0

These APIs were added on `main` after 1.0.0 and changed before 2.0.0, so
they affect only code built from git in between:

- `ProgramBuilder::add_block_in(lib, name)` is now `add_block_in(name, lib)`.
  Old calls still compile and fail at runtime with
  `RegistryError::BlockNotFound` ([#27]).
- `ProgramBuilder`, `ProgramBlockBuilder`, `BlockRef`,
  `ProgramBuilder::build` and `ConnectorHandle::new` are `#[must_use]`
  ([#26]).
- `ConnectorError::Subscribe`, `Publish` and `Request` gained a `source`,
  `Transport(String)` became `Transport { detail, source }`, and all four are
  `#[non_exhaustive]` ([#26]). `ConnectorError` is no longer `UnwindSafe`
  ([#26]).
- The connector messages became the `AddConnector`, `RemoveConnector` and
  `ManagedConnectors` requests, with typed errors ([#29]).
- `ExternalIn` gained its `timeout` input, and `ExternalOut` and `Request`
  count their deadline from the first issue of an operation ([#23]).
- The `ExternalIn`, `ExternalOut` and `Request` fault reasons render the
  connector error's whole source chain ([#26]).
- `JsConnector::from_js` returns `ExternalError` instead of `String` ([#26]).
- `ValueError::ZincDecode` no longer converts from `std::io::Error`, and
  `ZincDecode`, `ZincEncode` and `ParseBool` keep their cause in `source()`
  rather than in the message ([#26]).
- `EngineError::ScheduleRequiresSend` is removed: a block that is not
  `Send + Sync` is a compile error on `Engine::schedule` ([#18]).
- `EngineError::BlockRequestRejected` is removed, replaced by
  `InputNotFound`/`OutputNotFound` and `EngineError::Connect` ([#24]).
- `base::error` and its submodules are removed: the errors are at the crate
  root only ([#27]).
- `blocks::registry::eval_registered` is crate-private: use
  `eval_static_block` ([#27]).
- `EngineSessionOptions.watchSlots` is removed ([#29]) and `sleepDuration` is
  renamed `scanInterval` ([#30]). `EngineSession.watch` resolves with a
  `WatchSubscription` ([#30]), and `EngineSession.reset` and `stop` resolve
  once the engine has applied them ([#29]). Once the engine has stopped,
  however it was stopped, `stop` resolves and `reset` rejects instead of
  waiting forever ([#30]).
- `add_connector` and `connector_names` moved from the engines onto the
  `Engine` trait ([#27]), and `connector_names` is renamed
  `managed_connectors` ([#29]).
- `SingleThreadedEngine::schedule_local`, added in [#18], returns a `Result`
  ([#22]).
- The rustdoc anchor `base::engine#lifecycle-messages` is now
  `base::engine#lifecycle-requests` ([#29]).

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

[#5]: https://github.com/rracariu/logic-mesh/pull/5
[#7]: https://github.com/rracariu/logic-mesh/pull/7
[#11]: https://github.com/rracariu/logic-mesh/pull/11
[#12]: https://github.com/rracariu/logic-mesh/pull/12
[#13]: https://github.com/rracariu/logic-mesh/pull/13
[#17]: https://github.com/rracariu/logic-mesh/pull/17
[#18]: https://github.com/rracariu/logic-mesh/pull/18
[#20]: https://github.com/rracariu/logic-mesh/pull/20
[#21]: https://github.com/rracariu/logic-mesh/pull/21
[#22]: https://github.com/rracariu/logic-mesh/pull/22
[#23]: https://github.com/rracariu/logic-mesh/pull/23
[#24]: https://github.com/rracariu/logic-mesh/pull/24
[#26]: https://github.com/rracariu/logic-mesh/pull/26
[#27]: https://github.com/rracariu/logic-mesh/pull/27
[#28]: https://github.com/rracariu/logic-mesh/pull/28
[#29]: https://github.com/rracariu/logic-mesh/pull/29
[#30]: https://github.com/rracariu/logic-mesh/pull/30
[#32]: https://github.com/rracariu/logic-mesh/pull/32
[#33]: https://github.com/rracariu/logic-mesh/pull/33
[#34]: https://github.com/rracariu/logic-mesh/pull/34
[#35]: https://github.com/rracariu/logic-mesh/pull/35
[#36]: https://github.com/rracariu/logic-mesh/pull/36
[Unreleased]: https://github.com/rracariu/logic-mesh/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/rracariu/logic-mesh/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/rracariu/logic-mesh/releases/tag/v1.0.0
