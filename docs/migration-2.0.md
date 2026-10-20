# Migrating from logic-mesh 1.x to 2.0

logic-mesh 2.0 settles the API before it is locked for the 2.x series. Most
of the changes are mechanical: imports move to one canonical path, calls that
could fail return a `Result`, the hand-built engine messages give way to a
typed client, and a few structs gain constructors in place of struct
literals. This guide lists every breaking change for the Rust crate and the
npm package, most widely felt first. The [CHANGELOG](../CHANGELOG.md) has the
complete list, with the pull request each change came from.

Each section says what changed and why, then shows 1.x code and its 2.0
replacement. The 2.0 Rust snippets compile against logic-mesh 2.0: they are
part of the crate's doctests.

Some APIs were added on `main` after 1.0.0 and changed again before 2.0, such
as connectors, `ProgramBuilder` and `EngineSession`. They were never in a
release. If you used them from git, read
[APIs added after 1.0](#10-apis-added-after-100) at the end as well.

## Checklist

**Rust**

- [ ] Build with Rust 1.88 or newer.
- [ ] Depend on `logic-mesh = "2.0"`, and enable on your own `tokio` the
      runtime features you use (`rt`, `macros`; `rt-multi-thread` for the
      multi-threaded engine).
- [ ] Import through the canonical paths, or `use logic_mesh::prelude::*`.
      Errors live at the crate root.
- [ ] Handle the `Result` that `Engine::schedule` and the registry functions
      now return, and keep the engine that `run()` hands back.
- [ ] Pass a library, or `None`, to `schedule_block`,
      `schedule_block_with_uuid` and `eval_static_block`.
- [ ] Replace `set_sleep_dur` with `Engine::set_scan_interval`.
- [ ] Replace `EngineMessage` and `create_message_channel` with
      `Engine::client()` and `EngineClient`.
- [ ] Replace `anyhow` and string errors with `logic_mesh::Error`,
      `RequestError` and `ErrorCode`.
- [ ] Build `BlockDesc`, `Program` and the other `#[non_exhaustive]` structs
      with their constructors, and add wildcard arms to matches on the
      `#[non_exhaustive]` enums.
- [ ] Replace pin and link field access (`pin.val = …`, `out.value`) with the
      accessors.
- [ ] Check saved programs against the new load checks. 1.x saves load as
      they are.

**JavaScript / TypeScript**

- [ ] Install `zod` 4.6 or newer next to `logic-mesh`, on Node.js `^22.19.0`
      or `>=24.5.0`.
- [ ] Start the engine with `startEngine()` or `createEngineSession()`.
- [ ] Branch on `err.code` of the `LogicMeshError` a command rejects with.
- [ ] Close watches with the `WatchSubscription` that `session.watch()`
      resolves with.
- [ ] Pass the scan interval as a `number` of milliseconds (`scanInterval`).
- [ ] Read `getProgram()` and `inspectBlock()` results as plain objects.
- [ ] A Rust application that builds logic-mesh for `wasm32` with its own
      bindings: turn off the `js` feature.

## 1. Installing

### Rust 1.88 or newer

The crate declares `rust-version = "1.88"`: it uses let-chains, and several of
its dependencies need 1.88 too. Older toolchains get a clear Cargo error
instead of compile errors.

### Cargo dependencies and tokio features

**What changed.** logic-mesh used to enable tokio's `rt-multi-thread` on
every native build. An application's `#[tokio::main]` (multi-thread flavor)
or `Runtime::new()` compiled only because of that. logic-mesh 2.0 enables
only `rt`, `sync`, `time` and `macros`, with or without the `multi-threaded`
feature.

**Why.** Which runtime drives an engine is the application's choice, and
only applications that run the multi-threaded engine need tokio's
work-stealing scheduler. The library cannot enable it for them either: tokio
rejects `rt-multi-thread` on wasm32, and Cargo cannot enable a dependency
feature for one target only.

Before:

```toml
[dependencies]
logic-mesh = "1.0"
tokio = "1"
```

After, for the single-threaded engine:

```toml
[dependencies]
logic-mesh = "2.0"
tokio = { version = "1", features = ["rt", "macros"] }
```

and run it on a current-thread runtime, since its blocks run on the thread
that drives it:

```rust,no_run
use logic_mesh::{blocks::math::Add, prelude::*, single_threaded::SingleThreadedEngine};

#[tokio::main(flavor = "current_thread")]
async fn main() -> logic_mesh::Result<()> {
    let mut engine = SingleThreadedEngine::new();
    engine.schedule(Add::new())?;
    engine.run().await;
    Ok(())
}
```

For the multi-threaded engine, enable the feature, and `rt-multi-thread` on
your own tokio:

```toml
[dependencies]
logic-mesh = { version = "2.0", features = ["multi-threaded"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

The engine spawns its blocks onto the runtime it is called from. Scheduling
outside a tokio runtime returns `EngineError::NoRuntime` instead of panicking.

### wasm32 applications: the `js` feature

**What changed.** The JavaScript bindings (`logic_mesh::wasm`, every
wasm-bindgen export, and the module start hook that installs a panic hook and
a console logger) are behind a new `js` feature, which is on by default.

**Why.** A Rust application that built logic-mesh for wasm32 got all of the
npm package's exports, and a start hook that panicked if the application had
already installed a logger.

Nothing changes for the npm package or for a wasm32 build that wants the
bindings. A wasm32 application with bindings of its own turns them off:

```toml
[dependencies]
logic-mesh = { version = "2.0", default-features = false }
```

Without `js`, the engine still runs on wasm32, but it has no JavaScript host,
so it rejects external (JS) blocks with `ExternalError::Unsupported`, as
native builds do.

### Dependency versions

- libhaystack moved from 3.1 to 4.0. Its types appear throughout the API,
  so code that names them must use libhaystack 4. The crate re-exports it as
  `logic_mesh::libhaystack`, next to `logic_mesh::Value` and
  `logic_mesh::HaystackKind`, so you can name them without a direct
  dependency.
- `serde_json` is now a normal dependency, because block editor metadata is a
  `serde_json` object. It is re-exported as `logic_mesh::serde_json`.
- The tokio channel modules in the engine API are re-exported as
  `logic_mesh::sync::{mpsc, watch}`.
- `logic_mesh_block_macro` is pinned to exactly the same version as
  `logic-mesh`, because its generated code calls into an interface that only
  that release provides. Use the macros through `logic-mesh`
  (`logic_mesh::block`, `logic_mesh::BlockProps` or the prelude), not as a
  dependency of your own.

### npm: zod, Node.js and ESM

- **zod is a peer dependency**, `^4.6.0`. `defineBlock` maps your zod
  schemas with `instanceof` checks, so it must use the same zod your code
  imports. Install it yourself:

  ```sh
  npm install logic-mesh zod
  ```

  npm refuses to install the package next to zod 3 or a zod 4 older than 4.6
  (`ERESOLVE … peer zod@"^4.6.0"`), even in applications that never call
  `defineBlock`. Upgrade zod first.

- **Node.js `^22.19.0` or `>=24.5.0`** (`engines.node`). Those releases load
  the package with a plain `import`. Older ones need
  `--experimental-wasm-modules`.
- **ESM only.** From CommonJS, use `await import('logic-mesh')`.
  `require('logic-mesh')` works only on Node.js 26.
- **`sideEffects`** now lists only the wasm glue. A bundle that imports only
  `defineBlock`, `TypedBlock` or types no longer instantiates the wasm module.
  If you relied on a bare `import 'logic-mesh'` to instantiate it, import an
  engine API instead (`startEngine`, `createEngineSession`, `initEngine`, …).

The package README covers the bundler setup (Vite needs `vite-plugin-wasm`,
webpack 5 needs `asyncWebAssembly`) and the TypeScript `lib` setting.

## 2. Imports, canonical paths and the prelude

**What changed.** Every public item has exactly one path, and
`logic_mesh::prelude` gathers what defining, wiring and running a block takes.

**Why.** Most items were reachable through two or more paths, each of them a
semver commitment, and a missing trait import showed up as "no method named
…" rather than a missing name.

The prelude holds the traits `Block`, `BlockConnect`, `BlockProps`,
`BlockStaticDesc`, `Engine`, `InputProps`, `InputReader`, `Output` and
`OutputProps`, the pin types `InputImpl` and `OutputImpl`, and the `block` and
`BlockProps` macros.

Before:

```rust,ignore
use logic_mesh::base::block::{connect::connect_output, desc::BlockDesc, Block, BlockProps};
use logic_mesh::base::input::{input_reader::InputReader, props::InputProps};
use logic_mesh::base::output::props::OutputProps;
use logic_mesh::base::status::Status;
use logic_mesh::base::program::data::Program;
use logic_mesh::blocks::math::add::Add;
```

After:

```rust
use logic_mesh::base::Status;
use logic_mesh::base::block::{BlockDesc, connect_output};
use logic_mesh::base::program::Program;
use logic_mesh::blocks::math::Add;
use logic_mesh::prelude::*;
```

The paths that moved:

| 1.x | 2.0 |
|---|---|
| `base::block::connect::{BlockConnect, connect_output, connect_input, disconnect_output, disconnect_input}` | `base::block::{…}` |
| `base::block::connect::{disconnect_block, disconnect_link}` | Removed. Use `BlockConnect::disconnect_input` / `disconnect_output`, or `EngineClient::disconnect` on a running engine |
| `base::block::desc::{BlockDesc, BlockPin, BlockStaticDesc, BlockImplementation, BlockRunCondition}` | `base::block::{…}` |
| `base::block::props::{BlockProps, BlockInput, BlockOutput}` | `base::block::{…}` |
| `base::input::input_reader::InputReader`, `base::input::base::BaseInput`, `base::input::props::InputProps` | `base::input::{InputReader, BaseInput, InputProps}` |
| `base::output::base::BaseOutput`, `base::output::props::{OutputProps, OutDesc}` | `base::output::{BaseOutput, OutputProps, OutDesc}` |
| `base::link::base::BaseLink` | `base::link::BaseLink` |
| `base::status::Status` | `base::Status` |
| `base::program::data::{LinkData, PinValue, Position, Program, ProgramBlock}` | `base::program::{…}` |
| `base::program::BlockData` | Crate-private |
| `base::program::ProgramMeta` | Removed. Use `Program::name` and `Program::description` |
| `blocks::<category>::<module>::<Block>`, e.g. `blocks::math::add::Add` | `blocks::<category>::<Block>`, e.g. `blocks::math::Add` |
| `single_threaded::{BlockHandle, Messages}`, `multi_threaded::{BlockHandle, Messages}`, `multi_threaded::engine::*` | `base::engine::BlockHandle`; `Messages` is removed (see [section 4](#4-driving-a-running-engine-engineclient)); the engines stay at `single_threaded::SingleThreadedEngine` and `multi_threaded::MultiThreadedEngine` |
| `blocks::utils::{set_sleep_dur, get_sleep_dur}` | Removed. Use the per-engine scan interval (see [section 3](#3-building-and-running-engines)) |
| `blocks::registry::{eval_block_impl, eval_registered}` | Crate-private. Use `blocks::registry::eval_static_block` |
| `register_blocks!` | Crate-private. Register a block type with `blocks::registry::register::<B>()` |
| Errors (2.0 only; 1.x used `anyhow`) | Crate root only: `logic_mesh::{Error, Result, EngineError, …}` |

## 3. Building and running engines

### Scheduling returns a `Result`

**What changed.** `Engine::schedule` returns `logic_mesh::Result<()>`. It
rejects a block whose id is already scheduled with
`EngineError::DuplicateBlockId`, where 1.x replaced the scheduled block and
left its actor running orphaned. On native targets the block must be
`Send + Sync`. A block that is not goes to
`SingleThreadedEngine::schedule_local`. On the multi-threaded engine,
`schedule_send` is gone: `schedule` does its job.

Before:

```rust,ignore
let mut engine = SingleThreadedEngine::new();
engine.schedule(add);
let mut mt = MultiThreadedEngine::new();
mt.schedule_send(sine);
```

After:

```rust
use logic_mesh::{blocks::math::Add, prelude::*, single_threaded::SingleThreadedEngine};

fn configure() -> logic_mesh::Result<SingleThreadedEngine> {
    let mut engine = SingleThreadedEngine::new();
    let add = Add::new();
    let id = *add.id();
    engine.schedule(add)?;
    assert!(engine.block_handle(&id).is_some());
    Ok(engine)
}
```

`Engine::schedule_program_blocks` checks the whole program before it
schedules anything (see [section 8](#8-the-save-format)).

### `run()` consumes the engine and hands it back

**What changed.** `Engine::run` takes `self` and resolves to the idle engine
once the engine shuts down. Blocks, links, pin values and managed connectors
survive the shutdown, and running the returned engine again resumes them.
`engine.run().await` still compiles, but code that used `engine` after it
must use the returned value. The future is the trait's associated type
`Engine::RunFuture`, so generic code can require `E::RunFuture: Send` to
spawn it.

**Why.** The configuration phase (`Idle`) and the event loop (`Running`) are
now types, so configuration code cannot reach running-phase operations.

Before:

```rust,ignore
engine.run().await;
let handles = engine.block_handles();
```

After:

```rust
use logic_mesh::{prelude::*, single_threaded::SingleThreadedEngine};

async fn run_once(engine: SingleThreadedEngine) -> SingleThreadedEngine {
    // Resolves when a client sends `shutdown`.
    let engine = engine.run().await;
    println!("{} blocks kept", engine.block_handles().len());
    engine
}
```

### Configuration goes through the `Engine` trait

**What changed.** `add_connector`, `connector_names`, `block_handles`,
`block_handle`, `set_block_metadata`, `set_block_editor` and the scan
interval methods are `Engine` trait methods, implemented by both engines.
Calling them needs the trait in scope (`logic_mesh::prelude::*` or
`logic_mesh::base::engine::Engine`). `set_block_metadata` returns
`EngineError::BlockInstanceNotFound` for an unknown id instead of ignoring
it. The trait is sealed: only this crate's engines implement it, so methods
can be added to it in 2.x.

### The scan interval is per engine

**What changed.** 1.x had one process-wide sleep duration,
`blocks::utils::set_sleep_dur`. Each engine now owns its scan interval: the
period at which polling blocks (the timers, `Sequencer`, `Runtime`, an
`ExternalIn` waiting to resubscribe) check their state, and the shortest wait
of a block with no linked inputs. The default is still 200 ms, kept in whole
milliseconds.

Before:

```rust,ignore
logic_mesh::blocks::utils::set_sleep_dur(50);
let engine = SingleThreadedEngine::new();
```

After:

```rust
use std::time::Duration;
use logic_mesh::{prelude::*, single_threaded::SingleThreadedEngine};

let mut engine = SingleThreadedEngine::new().with_scan_interval(Duration::from_millis(50));
assert_eq!(engine.scan_interval(), Duration::from_millis(50));
engine.set_scan_interval(Duration::from_millis(100));
```

A block evaluated outside an engine (`eval_static_block`) always uses 200 ms.

### What the engines now do differently

No code change is needed for these, but behaviour you may rely on changed:

- **Execution only inside `run()`.** Blocks execute only while the engine runs
  and is not paused. The multi-threaded engine used to start executing blocks
  from `schedule()`. A run first starts the managed connectors and wires the
  links queued while idle, so no block runs against a half-built graph.
- **Shutdown keeps the graph** on both engines. The multi-threaded engine
  used to terminate every block on shutdown, so watchers no longer get a
  `Terminated` state for each block then.
- **Pause** stops block execution, and every request is still answered. The
  multi-threaded engine used to keep executing and drop requests. The
  single-threaded one kept executing.
- **A panicking block is isolated** (native targets). It enters
  `BlockState::Fault` with reason `block panicked: <message>`, its outputs
  and the blocks downstream fault too, and it is retried after 1 s, doubling
  up to 60 s. Before, its actor died silently and every later request to it
  failed. On wasm32 a panic still aborts the instance.
- **A pin write runs the block.** Writing an input through the engine now
  runs one `execute` cycle with the new value, linked or not, and the value
  is kind-converted like a linked one. A value of the wrong kind faults the
  block.
- **A fault clears only after a completed cycle.** A request, pause or
  shutdown that cuts a cycle short leaves the block in `Fault`.
- **Link ids are kept.** A connect request keeps the `LinkData::id` it is
  given, where 1.x always made up a fresh one, and rejects an id that is
  taken (`DuplicateLinkId`) or not a UUID (`InvalidLinkUuid`). A link to a
  pin that does not exist is rejected up front with `PinNotFound`.
- **A closed mailbox ends its actor** instead of spinning, and dropping an
  engine ends all of its actors.

## 4. Driving a running engine: `EngineClient`

**What changed.** `EngineMessage` and its `Messages` alias are replaced by
`EngineRequest` and `EngineResponse`, which have named fields and are
`#[non_exhaustive]`. Each response is correlated to its request by id, so any
number of requests can be in flight, and every request is answered exactly
once, after it took effect, including `Pause`, `Resume`, `Reset` and
`Shutdown`. Most code uses `EngineClient`, which has an async method per
request:

| 1.x message | `EngineClient` method | `EngineRequest` |
|---|---|---|
| `AddBlockReq` | `add_block(name, block_id, library)` | `AddBlock` |
| `RemoveBlockReq` | `remove_block(block)` | `RemoveBlock` |
| `ConnectBlocksReq` | `connect(link)` | `Connect` |
| `RemoveLinkReq` | `disconnect(link)` | `Disconnect` |
| `WriteBlockInputReq` | `write_input(block, pin, value)` | `WriteInput` |
| `WriteBlockOutputReq` | `write_output(block, pin, value)` | `WriteOutput` |
| `InspectBlockReq` | `inspect_block(block)` | `InspectBlock` |
| `EvaluateBlockReq` | `eval_block(name, inputs, library)` | `EvalBlock` |
| `GetCurrentProgramReq` | `save_program()` | `SaveProgram` |
| `LoadProgramReq` | `load_program(program)` | `LoadProgram` |
| `WatchBlockSubReq` | `watch()` | `Watch` |
| `WatchBlockUnsubReq` | `unwatch(watch_id)` | `Unwatch` |
| `Pause`, `Resume`, `Reset`, `Shutdown` | `pause()`, `resume()`, `reset()`, `shutdown()` | `Pause`, `Resume`, `Reset`, `Shutdown` |
| (new) | `set_block_metadata`, `set_block_editor` | `SetBlockMetadata`, `SetBlockEditor` |
| (new) | `add_connector`, `remove_connector`, `list_connectors` | `AddConnector`, `RemoveConnector`, `ListConnectors` |

**Why.** Requests were tuples with positional fields, the caller picked the
sender id, replies were matched by order (one request in flight per channel),
control messages got no reply, and errors were strings.

Before:

```rust,ignore
let (reply_tx, mut reply_rx) = tokio::sync::mpsc::channel(32);
let sender_id = Uuid::new_v4();
let engine_tx = engine.create_message_channel(sender_id, reply_tx);

tokio::spawn(async move {
    engine_tx
        .send(EngineMessage::AddBlockReq(sender_id, "Add".into(), None, None))
        .await
        .unwrap();
    if let Some(EngineMessage::AddBlockRes(Ok(id))) = reply_rx.recv().await {
        engine_tx
            .send(EngineMessage::WriteBlockInputReq(sender_id, id, "in0".into(), 2.into()))
            .await
            .unwrap();
        let _ = reply_rx.recv().await;
    }
    engine_tx.send(EngineMessage::Shutdown).await.unwrap();
});
engine.run().await;
```

After:

```rust
use logic_mesh::{RequestError, prelude::*, single_threaded::SingleThreadedEngine};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), RequestError> {
    let mut engine = SingleThreadedEngine::new();
    // Create clients while the engine is idle; clone one for more handles.
    let client = engine.client();

    let driver = async move {
        let id = client.add_block("Add", None, None).await?;
        client.write_input(id, "in0", 2.into()).await?;
        let block = client.inspect_block(id).await?;
        assert_eq!(block.name, "Add");
        // Answered once execution and the connectors have stopped.
        client.shutdown().await
    };
    let (_engine, driven) = tokio::join!(engine.run(), driver);
    driven
}
```

On native targets an `EngineClient` is `Send + Sync`, so it can drive a
multi-threaded engine from any task:

```rust,ignore
use logic_mesh::{blocks::math::Add, multi_threaded::MultiThreadedEngine, prelude::*};

#[tokio::main] // a multi-thread runtime: needs tokio's `rt-multi-thread`
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = MultiThreadedEngine::new();
    engine.schedule(Add::new())?;
    let client = engine.client();
    let running = tokio::spawn(engine.run());

    let program = client.save_program().await?;
    println!("{} blocks", program.blocks.len());

    client.shutdown().await?;
    let _engine = running.await?;
    Ok(())
}
```

The multi-threaded engine's public async methods from 1.x
(`inspect_block`, `write_input`, `write_output`, `connect_blocks`,
`remove_block`, `save_program`, `load_program`, `disconnect_link_by_id`) are
gone too: use the client's methods. So is its synchronous
`add_block(name, id, lib)`. Schedule a registered block on the idle engine
with `blocks::registry::schedule_block` or `schedule_block_with_uuid`, or add
it to a running engine with `EngineClient::add_block`.

Before:

```rust,ignore
let mut mt = MultiThreadedEngine::new();
let id = mt.add_block("Add".to_string(), None, None)?;
```

After:

```rust,ignore
use logic_mesh::{Uuid, blocks::registry::schedule_block, multi_threaded::MultiThreadedEngine};

// Call it inside a tokio runtime: the block's actor is spawned on it.
fn add(mt: &mut MultiThreadedEngine) -> logic_mesh::Result<Uuid> {
    schedule_block("Add", None, mt)
}
```

### Watches

**What changed.** `watch()` opens a new watch each time and returns a
`Watch`: its engine-assigned `WatchId` and the receiver its
`WatchMessage`s arrive on. `unwatch(id)` closes it. Watch ids are
engine-wide, so any client may close any watch. In 1.x a channel had one
watcher, and subscribing again replaced it.

```rust
use logic_mesh::{RequestError, base::engine::EngineClient};

async fn print_changes(client: &EngineClient) -> Result<(), RequestError> {
    let mut watch = client.watch().await?;
    if let Some(change) = watch.notifications.recv().await {
        println!("block {} is {}", change.block_id, change.state.label());
    }
    client.unwatch(watch.id).await?;
    Ok(())
}
```

### Custom transports

`Engine::create_message_channel` now takes the response sender and returns
the engine-assigned `ClientId` with the request sender:
`fn create_message_channel(&mut self, responses: ResponseSender) -> (ClientId, RequestSender)`.
Requests travel in an `Envelope { client, id, body }` (`RequestMessage`), and
responses come back in one carrying the same ids (`ResponseMessage`). The
`Engine::Channel` associated type is removed. `EngineClient::new` wraps the
two ends of such a channel, and `EngineClient::request` sends a raw
`EngineRequest`. The `base::engine::messages` module docs describe the
protocol.

## 5. Typed errors and error codes

### `anyhow` is replaced by `logic_mesh::Error`

**What changed.** 1.x returned `anyhow::Result` and engine replies carried
`String`s. 2.0 returns typed errors, defined with `thiserror` and re-exported
at the crate root:

- `Error`, the aggregate the entry points return, and `Result<T>`;
- the per-subsystem enums it wraps: `RegistryError`, `EngineError`,
  `ConnectError`, `ValueError`, `ExternalError` and `ConnectorError`, plus
  `BlockDescError`;
- `RequestError`, which an engine answers a failed request with.

All of them are `#[non_exhaustive]`. `Error` implements `std::error::Error`,
so `?` into `anyhow::Error` or `Box<dyn Error>` keeps working.

Before:

```rust,ignore
match schedule_block("Add", &mut engine) {
    Ok(id) => println!("{id}"),
    Err(err) if err.to_string().contains("not found") => println!("unknown block"),
    Err(err) => return Err(err),
}
```

After:

```rust
use logic_mesh::{
    Error, RegistryError, blocks::registry::schedule_block, single_threaded::SingleThreadedEngine,
};

fn add_block(engine: &mut SingleThreadedEngine, name: &str) -> logic_mesh::Result<()> {
    match schedule_block(name, None, engine) {
        Ok(id) => println!("{id}"),
        Err(Error::Registry(RegistryError::BlockNotRegistered { .. })) => {
            println!("no library has a block named {name}");
        }
        Err(err) => return Err(err),
    }
    Ok(())
}
```

### Request errors carry a code

A failed request answers with a `RequestError`:

- `code`, a stable `ErrorCode` to branch on, written as its variant name
  (`"BlockInstanceNotFound"`);
- `message`, the error's message followed by its whole source chain;
- `error()`, the typed `Error` within the process, to match on its data.

Every error type has a `code()` method. `ErrorCode` is `#[non_exhaustive]`,
and a code from a newer release reads as `ErrorCode::Unknown`.

```rust
use logic_mesh::{ErrorCode, RequestError, Uuid, base::engine::EngineClient};

/// Removes `block`, treating a block that is already gone as removed.
async fn remove(client: &EngineClient, block: Uuid) -> Result<(), RequestError> {
    match client.remove_block(block).await {
        Err(err) if err.code == ErrorCode::BlockInstanceNotFound => Ok(()),
        other => other.map(|_| ()),
    }
}
```

A wrapping error keeps the error it wraps as its `source()`, not in its
message. Fault reasons and request messages render the whole chain, so they
still show the cause.

### Parsing block description fields

`BlockImplementation` and `BlockRunCondition` parse with `str::parse`
(`FromStr<Err = BlockDescError>`). Their `TryFrom<&str, Error = String>`
impls are removed.

```rust
use logic_mesh::base::block::BlockRunCondition;

fn main() -> Result<(), logic_mesh::BlockDescError> {
    let condition: BlockRunCondition = "always".parse()?;
    assert_eq!(condition, BlockRunCondition::Always);
    Ok(())
}
```

## 6. Defining blocks

### The block macros

The macros still take the same attributes. A complete 2.0 block:

```rust
use logic_mesh::prelude::*;

/// Doubles its input.
///
/// Values that are not numbers are ignored.
#[block]
#[derive(BlockProps, Debug)]
#[category = "math"]
#[library = "my_blocks"]
#[ver = "1.1.0"]
struct Double {
    #[input(name = "in", kind = "Number")]
    input: InputImpl,
    #[output(kind = "Number")]
    out: OutputImpl,
}

impl Block for Double {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;
        if let Some(Ok(num)) = self.input.get_value().map(f64::try_from) {
            self.out.set((num * 2.0).into());
        }
    }
}

let desc = Double::static_desc();
assert_eq!(desc.ver, "1.1.0");
assert_eq!(desc.doc, "Doubles its input.\n\nValues that are not numbers are ignored.");
```

What changed:

- **Misuse is a compile error, spanned on the offending tokens**, instead of
  a "proc-macro panicked" or an error in generated code. This covers:
  arguments to `#[block]`; a missing `#[category]` or output; an unknown,
  duplicate or non-string attribute or pin argument; a pin attribute on a
  field that is not an `InputImpl` / `OutputImpl`; a struct-level
  `#[output]`; a block-level `#[input]` without an integer `count`; a kind
  that is not an identifier; and a pin name used twice on a block.
  1.x silently ignored or merged most of these.
- **`#[block]` takes no arguments.** A renamed `logic-mesh` dependency is
  still named with `#[logic_mesh(crate = "path")]`.
- **`#[ver = "…"]` is registered**; 1.x read it and dropped it. The default
  stays `1.0.0`.
- **Doc comments keep their lines.** `desc.doc` joins the `///` lines with
  `\n` and keeps blank lines. 1.x glued them together.
- **A bare `#[input]` / `#[output]`** declares a pin named after its field, of
  kind `Null`. 1.x ignored it.
- **A raw field name** (`r#in`) names the pin without its prefix (`in`).
- **Other attributes are left alone.** Only `dis`, `library`, `category`,
  `ver` and the doc comment are read from the struct. 1.x took every
  `#[name = "…"]` attribute for a description field, so a
  `#[must_use = "…"]` broke the build.

### Hand-written descriptions use builders

**What changed.** `BlockDesc`, `BlockPin` and `OutDesc` are
`#[non_exhaustive]`, so they cannot be built with struct literals outside the
crate. Their fields stay public to read and assign. `BlockImplementation` and
`BlockRunCondition` are `#[non_exhaustive]` too.

**Why.** A struct literal in every downstream block meant `BlockDesc` could
never gain a field.

Before:

```rust,ignore
let desc = BlockDesc {
    name: "Scale".into(),
    library: "custom".into(),
    dis: "Scale".into(),
    category: "math".into(),
    ver: "1.0.0".into(),
    inputs: vec![BlockPin { name: "in".into(), kind: HaystackKind::Number }],
    outputs: vec![BlockPin { name: "out".into(), kind: HaystackKind::Number }],
    doc: "Scales its input".into(),
    implementation: BlockImplementation::Native,
    run_condition: None,
};
```

After:

```rust
use logic_mesh::{HaystackKind, base::block::BlockDesc};

let desc = BlockDesc::new("Scale", "custom")
    .with_category("math")
    .with_doc("Scales its input")
    .with_input("in", HaystackKind::Number)
    .with_output("out", HaystackKind::Number);
assert_eq!(desc.dis, "Scale");
assert_eq!(desc.ver, BlockDesc::DEFAULT_VER);
```

`BlockPin::new(name, kind)` and `OutDesc::new(name, kind)` build the pin
descriptors.

### Block traits

- `Block` requires only `BlockProps`, not `BlockConnect` and through it
  `BlockStaticDesc`, so a block whose description exists only at runtime can
  implement it. Code that calls `B::static_desc()` on a `B: Block` adds a
  `BlockStaticDesc` bound. Registering a type by name still needs one.
- `BlockStaticDesc::desc()` is renamed `static_desc()`, so it no longer
  shares a name with `BlockProps::desc(&self)`.
- `BlockConnect` has a blanket impl for every `BlockProps` type and cannot be
  implemented by hand. Its methods, and the free functions `connect_output`,
  `connect_input`, `disconnect_output` and `disconnect_input`, return
  `Result<_, ConnectError>` instead of `Result<_, &'static str>`. Its variants
  name both ends of the link: `OutputNotFound`, `InputNotFound`,
  `AlreadyConnected`, `SelfLink` and `NotConnected`.

```rust
use logic_mesh::{ConnectError, base::block::connect_output, blocks::math::Add, prelude::*};

fn main() -> Result<(), ConnectError> {
    let mut source = Add::new();
    let mut target = Add::new();
    connect_output(&mut source.out, target.inputs_mut()[0])?;
    let again = source.connect_output("out", target.inputs_mut()[0]);
    assert!(matches!(again, Err(ConnectError::AlreadyConnected { .. })));
    Ok(())
}
```

### The registry

- The functions that build a block by name take its library as their
  second argument: `schedule_block(name, lib, engine)`,
  `schedule_block_with_uuid(name, lib, uuid, engine)` and
  `eval_static_block(name, lib, inputs)`. Pass `None` to search every
  library. `schedule_block_send` and `schedule_block_send_with_uuid` are
  removed, because `schedule_block` takes any engine.
- Registering a block type a second time fails with
  `RegistryError::BlockAlreadyRegistered`.
- `get_block`, `get_core_block` and `make` return
  `Result<_, RegistryError>` instead of `Option`. `register::<B>()` and
  `register_block_desc` return `Result<(), RegistryError>`, and
  `schedule_block`, `schedule_block_with_uuid` and `eval_static_block` return
  `logic_mesh::Result`.
- **A lookup without a library searches every library**, everywhere: in
  `get_block(name, None)`, `make`, the scheduling functions, an `add_block` or
  `eval_block` request, and the JS `addBlock` / `evalBlock`. 1.x looked only
  in `core` in some of these. A name that no library has is
  `BlockNotRegistered`, and a name more than one library has (a block of your
  own named like a core block, say) is `AmbiguousBlockName`. Pass the
  library to pick one. `Some(lib)` looks only in `lib` (`BlockNotFound`).
- `BlockEntry::make` is a method, `entry.make()`, instead of a field holding
  an optional constructor.

```rust
use logic_mesh::blocks::registry::get_block;

fn main() -> Result<(), logic_mesh::RegistryError> {
    let entry = get_block("Add", None)?;
    assert_eq!(entry.desc.library, "core");
    assert!(entry.make().is_some());
    Ok(())
}
```

### Exhaustive matches need a wildcard arm

`Status`, `BlockState`, `LinkState`, `ChangeSource`, `BlockImplementation`,
`BlockRunCondition`, `ErrorCode`, `EngineRequest`, `EngineResponse` and every
error enum are `#[non_exhaustive]`, so variants can be added in 2.x. A
`match` on one of them outside the crate needs a `_` arm:

```rust
use logic_mesh::base::Status;

fn describe(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Fault => "fault",
        _ => "other",
    }
}
```

## 7. Private fields and accessors

**What changed.** The fields of `BaseInput`, `BaseOutput` and `BaseLink` are
private to the crate.

**Why.** Writing `out.value` skipped the propagation `Output::set` does, and
the connection counts and link lists are engine bookkeeping. A stray write
broke `BitwiseNot`, which wrote its result into `out.value` so linked blocks
never received it.

| 1.x | 2.0 |
|---|---|
| `input.val = Some(value)` | `input.set(value)` (`InputImpl::set`, status `Ok`) or `Input::set_value(value, status)` |
| `input.val`, `input.status` | `InputProps::get_value()`, `Input::status()` |
| `input.connection_count` | `input.connection_count()` |
| `out.value = value` | `out.set(value)` (`Output::set`) |
| `out.value`, `out.links`, `out.block_id` | `OutputProps::value()`, `links()`, `block_id()` |
| `out.effective_status` | `out.effective_status()` |
| `BaseLink { id, target_block_id, target_input, tx, state }` | `BaseLink::connected(id, target_block_id, target_input, tx)` or `BaseLink::new`; read with the `Link` trait and `BaseLink::tx()` |

Before:

```rust,ignore
let mut sine = SineWave::new();
sine.amplitude.val = Some(3.into());
println!("{:?}", sine.out.value);
```

After:

```rust
use logic_mesh::{blocks::misc::SineWave, prelude::*};

let mut sine = SineWave::new();
sine.amplitude.set(3);
assert_eq!(sine.amplitude.get_value(), Some(&3.into()));
println!("{:?}", sine.out.value());
```

The structs engines produce are `#[non_exhaustive]` too, with public fields
to read: `WatchMessage`, `BlockDefinition`, `BlockInputData` and
`BlockOutputData`. A test double or a custom transport builds them with
`WatchMessage::new(block_id, changes, state)`,
`BlockDefinition::new(id, name, library)` and its `with_input`,
`with_output` and `with_state` methods, `BlockInputData::new(kind, val,
is_connected)` and `BlockOutputData::new(kind, val)`. Patterns on them need
`..`.

## 8. The save format

### Versioned programs, and 1.x saves

**What changed.** A saved `Program` starts with a format `version`. The
current format is 1 (`Program::FORMAT_VERSION`). A save without one, which
is every 1.x save, reads as version 0. Version 0 is version 1 without the new
optional fields and with the position keyed `positions`, so **a 1.x save
loads unchanged**, and saving it again writes version 1. A program from a
newer format fails to deserialize with the
`EngineError::UnsupportedProgramVersion` message before its blocks are read,
and an engine rejects one built in code with that error.

The JSON changes:

- `"version"` comes first.
- A block's `"positions"` is now `"position"`. `"positions"` is still read,
  and a block with both keys is an error.
- A block records its type's version in `"ver"`. A load compares it with the
  registered block: a different major version (or a different minor version
  below 1.0) fails with `EngineError::BlockVersionMismatch`, and any other
  difference logs a warning. Blocks without `"ver"` are not checked.
- A block may carry `"editor"`, an opaque JSON object the engine saves back
  unchanged (`EditorMetadata`, a `serde_json` map). Set it with
  `ProgramBlockBuilder::editor`, `Engine::set_block_editor` or a
  `SetBlockEditor` request.
- A link without an id no longer writes `"id": null`.

```rust
use logic_mesh::{base::program::Program, serde_json};

fn main() -> Result<(), serde_json::Error> {
    let saved_by_1_x = r#"{
        "name": "demo",
        "blocks": {
            "8e5c1c62-4a39-4c5b-9a4b-0f6e3b1f1a01": {
                "name": "Add", "lib": "core", "positions": { "x": 10, "y": 20 },
                "inputs": {}, "outputs": {}
            }
        },
        "links": {}
    }"#;
    let program: Program = serde_json::from_str(saved_by_1_x)?;
    assert_eq!(program.version, 0);
    let block = &program.blocks["8e5c1c62-4a39-4c5b-9a4b-0f6e3b1f1a01"];
    assert_eq!(block.position.map(|p| p.x), Some(10.0));
    Ok(())
}
```

### Program types have constructors

**What changed.** `Program`, `ProgramBlock`, `LinkData`, `PinValue` and
`Position` are `#[non_exhaustive]`, and `ProgramBlock::positions` is renamed
`position`. Build them with their constructors, then set the public fields.

Before:

```rust,ignore
let block = ProgramBlock {
    name: "Add".into(),
    lib: "core".into(),
    label: Some("sum".into()),
    positions: Some(Position { x: 10.0, y: 20.0 }),
    inputs: BTreeMap::new(),
    outputs: BTreeMap::new(),
};
let link = LinkData {
    id: None,
    source_block_uuid: a.to_string(),
    target_block_uuid: b.to_string(),
    source_block_pin_name: "out".into(),
    target_block_pin_name: "in0".into(),
};
```

After:

```rust
use logic_mesh::{
    Uuid,
    base::program::{LinkData, PinValue, Position, Program, ProgramBlock},
};

let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
let mut block = ProgramBlock::new("Add", "core");
block.label = Some("sum".into());
block.position = Some(Position::new(10.0, 20.0));
block.inputs.insert("in1".into(), PinValue::new(2, false));

let mut program = Program::new();
program.blocks.insert(a.to_string(), block);
program.blocks.insert(b.to_string(), ProgramBlock::new("Add", "core"));
let link_id = Uuid::new_v4();
program.links.insert(
    link_id.to_string(),
    LinkData::new(a, "out", b, "in0").with_id(link_id),
);
assert_eq!(program.version, Program::FORMAT_VERSION);
```

`ProgramBuilder` checks blocks, pin values and links against the registry as
you add them, and is the shorter way to build a program in code.

### Loads are all or nothing

**What changed.** `schedule_program_blocks` and `load_program` check the
whole program before they schedule any block, and a load that still fails
while wiring removes the program's blocks again. 1.x scheduled block by
block and left a failed load half applied. Programs that 1.x loaded may now
be rejected:

| Rejected with | When |
|---|---|
| `InvalidBlockUuid`, `DuplicateBlockId` | a block id does not parse, is already scheduled, or is named twice |
| `BlockNotFound`, `BlockNotRegistered`, `AmbiguousBlockName` | a block type does not resolve |
| `InputNotFound`, `OutputNotFound` | a saved pin value names a pin the block does not have. 1.x ignored it |
| `InvalidLinkUuid`, `DuplicateLinkId` | a link id (its `id`, or else its key in `links`) is not a UUID or is already used |
| `PinNotFound` | a link names a pin that does not exist |
| `LinkBlockNotInProgram` | a link joins a block outside the program. A program must be self-contained |
| `DuplicateLink` | two links join the same pins |
| `UnsupportedProgramVersion`, `BlockVersionMismatch` | see above |

What a load keeps also changed. Links keep their saved ids, so a saved link
id can be removed after a load. A save returns the loaded program's name and
description. `schedule_program_blocks` now applies the program's saved pin
values when `run()` starts.

## 9. The JavaScript API

### Start the engine with a session

**What changed.** The npm package wraps the start sequence in
`startEngine()` and `createEngineSession()`, which return an
`EngineSession`. The 1.x sequence still works, but it is easy to get wrong:
a command handle created after `run()` throws "recursive use of an object
detected", and a command awaited before `run()` deadlocks.

Before:

```ts
import { initEngine } from 'logic-mesh';

const engine = initEngine();
const command = engine.engineCommand();
engine.run();
const id = await command.addBlock('Add');
```

After:

```ts
import { startEngine } from 'logic-mesh';

const session = startEngine({ scanInterval: 100 });
const id = await session.command.addBlock('Add');
// ...
await session.stop();
```

Register JS blocks between `createEngineSession()` and `session.start()`:
block registration needs the engine object, which `run()` holds from the
moment it starts.

```ts
import { createEngineSession } from 'logic-mesh';

const session = createEngineSession();
MyBlock.register(session.engine);
session.start();
```

A command handle takes overlapping calls: each call's promise settles with
the engine's answer to it. 1.x threw "recursive use of an object detected" on
the second call.

### Errors are `LogicMeshError`s with a code

**What changed.** Commands used to reject with plain strings, such as
"Failed to send message". They now reject with an `Error` named
`LogicMeshError`, with a stable `code` (the `LogicMeshErrorCode` union) and
a message. The synchronous calls `initEngine`, `new BlocksEngine`,
`registerBlock`, `registerConnector` and `setLogLevel` throw them too.

```ts
import { isLogicMeshError } from 'logic-mesh';

try {
  await session.command.loadProgram(saved);
} catch (err) {
  if (isLogicMeshError(err) && err.code === 'UnsupportedProgramVersion') {
    console.warn('Saved by a newer logic-mesh:', err.message);
  } else {
    throw err;
  }
}
```

Malformed input is rejected instead of being guessed at:

- an unparseable block or link id rejects with `InvalidBlockUuid` /
  `InvalidLinkUuid`, where most commands used the nil UUID;
- a value that is not a Haystack value (a function, a `Symbol`, a `Date`, a
  class instance, a boxed primitive, a cycle) rejects with `InvalidValue`
  instead of being written as Null or as a Dict of its properties. Primitives,
  arrays, `Map`s and plain objects are read, and `null` and `undefined` still
  write Null;
- `loadProgram` rejects a newer format with `UnsupportedProgramVersion` and
  any other unreadable payload with `InvalidProgram`.

### Watches can be closed

**What changed.** `EngineCommand.createWatch(callback)` used to return a
promise that never resolved. It now resolves with the watch id once the
watch is open, and `removeWatch(id)` closes it. On a session,
`session.watch(callback)` resolves with a `WatchSubscription`: `close()`
stops the callback at once and closes the watch, and `await using` closes it
at the end of the scope. Once the session has stopped, `close()` and
`session.unwatch(id)` resolve `false`, and `watch()` rejects.

```ts
await using watch = await session.watch((notification) => {
  console.log(notification.id, notification.changes);
});
// ... closed when the scope ends, or earlier with `await watch.close()`.
```

### Control commands resolve once applied

`pauseExecution`, `resumeExecution`, `resetEngine` and `stopEngine`, and
`session.reset()` and `session.stop()`, resolve once the engine has applied
them, not once they were queued. A paused engine answers every command, and
only block execution stops.

### The scan interval is a number of milliseconds

`new BlocksEngine(scanInterval)` took a `bigint` and `initEngine` a `number`
that wrapped when negative. Both take a `number` of milliseconds, checked to
be finite and between 0 and `Number.MAX_SAFE_INTEGER` (fractions are
dropped), and throw `InvalidArgument` otherwise. Each sets the interval of
the engine it creates only, where 1.x set it for every engine in the module.
The session option is `scanInterval`.

```ts
import { initEngine } from 'logic-mesh';

const engine = initEngine(50); // was initEngine(50) or new BlocksEngine(50n)
```

### Plain objects, and typed declarations

- `getProgram()` resolves with plain JSON objects: `program.blocks[id]`
  works, and `program.blocks.get(id)` does not. Missing values are `null`.
- `inspectBlock()` resolves with a `BlockInspection` of plain objects:
  `{ id, name, lib, inputs: { [pin]: { kind, val, isConnected } },
  outputs: { [pin]: { kind, val } }, state, faultReason? }`. In 1.x it
  returned Maps keyed `library` and `fault_reason`.
- The generated declarations name the package's types instead of `any`:
  `loadProgram(program: Program)`, `getProgram(): Promise<Program>`,
  `createLink(): Promise<LinkData>`, `listBlocks(): BlockDesc[]`,
  `registerBlock(desc: BlockDesc, func)`,
  `createWatch(callback: (notification: BlockNotification) => void)`, and so
  on. Code that passed loosely typed values may need a type.
- The `Program` types follow the save format: `Program<Editor>` with
  `version`, optional `name`, and `ProgramBlock` with `ver`, an optional
  `position` (was `positions`) and `editor` metadata. The block-level
  `widget` field is gone: keep editor state in `editor`. `PinValue` requires
  both `value` and `isConnected`. A custom `Editor` metadata type must be
  declared with `type`, not `interface`.
- `LinkData` is a `type`, not an `interface`, so it can no longer be merged
  with a declaration of your own.

### Logging

The module start hook installs a console logger at **warn** level, keeps a
logger that is already installed, and no longer panics. 1.x logged at trace.
Change the level with `setLogLevel`:

```ts
import { setLogLevel } from 'logic-mesh';

setLogLevel('debug'); // 'off' | 'error' | 'warn' | 'info' | 'debug' | 'trace'
```

The level applies to the whole wasm module, including a logger a Rust
application installed.

### JS blocks

- An executor may return its outputs directly instead of through a Promise.
  A throw or a rejection faults the block with
  `JS execute failed: <error.toString()>`.
- `defineBlock` maps `.nullable()`, `.optional()` and `.default()` schemas to
  their inner kind, a literal to its value's kind, and a union to the kind
  its members share. Any other schema throws
  `Invalid kind: <Schema> has no pin kind`.
- `defineBlock` copies the description you pass instead of writing the pins
  into it, so one description can define several blocks.
- Block descriptions from `listBlocks()` can have multi-line `doc` strings
  (see [section 6](#the-block-macros)).

## 10. APIs added after 1.0.0

These APIs landed on `main` after 1.0.0 and changed before 2.0. Code written
against `main` in between needs these changes too.

**Connectors** (`base::connector`, added in [#13]):

- `ConnectorError::Subscribe`, `Publish` and `Request` gained an optional
  `source`, and `Transport(String)` became `Transport { detail, source }`.
  All four are `#[non_exhaustive]`: build them with
  `ConnectorError::subscribe(address, detail)`, `publish`, `request` or
  `transport(detail)`, and attach the protocol client's error with
  `.with_source(err)`. `ConnectorError` is no longer `UnwindSafe`.
- `ExternalIn` has a `timeout` input (milliseconds, default 5000), and a
  subscribe that does not settle in time faults the block.
- The `ExternalOut` and `Request` deadlines count from when the operation was
  first issued, so a stream of engine requests can no longer keep a slow
  operation from timing out.
- The connector messages became `AddConnector`, `RemoveConnector` and
  `ListConnectors` requests, with the typed errors `ConnectorNotFound`,
  `ConnectorAlreadyManaged`, `ConnectorNotManaged`, `ConnectorStartFailed`
  and `ConnectorStartTimeout`.
- `add_connector` and `connector_names` are `Engine` trait methods.
- `JsConnector::from_js` returns `ExternalError` instead of `String`.

**`ProgramBuilder`** (added in [#18]):

- `add_block_in(lib, name)` is now **`add_block_in(name, lib)`**, the order of
  every other lookup. Old calls still compile, and fail at runtime with
  `RegistryError::BlockNotFound { library: <name>, name: <lib> }`, so check
  every call site.
- The builder, `ProgramBlockBuilder`, `BlockRef` and `build()` are
  `#[must_use]`. A block you add and never link needs
  `let _ = builder.add_block(..)?.finish();`.
- `ProgramBlockBuilder::editor` sets a block's editor metadata, and
  `add_block` records the block's version.

**`EngineSession`** (added in [#13]):

- `EngineSessionOptions.watchSlots` is removed: `watch()` opens any number of
  watches, and resolves with a `WatchSubscription` rather than an id.
- `EngineSessionOptions.sleepDuration` is renamed `scanInterval`.
- `reset()` and `stop()` resolve once the engine has applied them.

**Typed errors** (added in [#11]):

- `ValueError::ZincDecode` no longer converts from `std::io::Error`, so `?`
  no longer turns an I/O error into a "Zinc decoding failed". Map it
  explicitly with `.map_err(ValueError::ZincDecode)`.
- `ZincDecode`, `ZincEncode` and `ParseBool` read "Zinc decoding failed",
  "Zinc encoding failed" and "Invalid boolean literal". The wrapped error is
  their `source()`, no longer repeated in the message.
- `EngineError::BlockRequestRejected(String)` is gone. A missing pin is
  `EngineError::InputNotFound` / `OutputNotFound { block, pin }`, and a link
  the block rejects is `EngineError::Connect(ConnectError)`.

**Engines** (changed in [#18]):

- `SingleThreadedEngine::schedule_local` returns `logic_mesh::Result<()>`.
- `EngineError::ScheduleRequiresSend` is removed: a block that is not
  `Send + Sync` is a compile error on `Engine::schedule`.

[#11]: https://github.com/rracariu/logic-mesh/pull/11
[#13]: https://github.com/rracariu/logic-mesh/pull/13
[#18]: https://github.com/rracariu/logic-mesh/pull/18
