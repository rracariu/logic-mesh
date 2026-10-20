// Copyright (c) 2022-2026, Radu Racariu.

//! The engine protocol: the requests an engine answers, its responses,
//! and the data they carry.
//!
//! A running engine is driven over message channels. Each
//! [`EngineRequest`] travels in an [`Envelope`] stamped with the
//! [`ClientId`] of the channel it came from and a [`RequestId`] the
//! client chose; the engine answers it with exactly one
//! [`EngineResponse`], in an envelope carrying the same two ids, on that
//! client's response channel. The request id is what correlates the two,
//! so a client can have any number of requests in flight.
//!
//! Every response carries a `result`: the request's outcome, or the
//! [`RequestError`] it failed with — a stable
//! [`ErrorCode`](crate::base::error::ErrorCode) and a message, and within
//! the process the typed error.
//!
//! [`EngineClient`](crate::base::engine::EngineClient) speaks the
//! protocol for Rust callers: an async method per request, returning the
//! matching response's result. The raw types here are for custom
//! transports; open a channel for one with
//! [`Engine::create_message_channel`](crate::base::engine::Engine::create_message_channel).
//!
//! # Connector routing
//!
//! Connector route/subscription management needs no dedicated
//! requests: rebinding an external block is a write to its
//! `connector` / `address` input pins through the existing
//! [`WriteInput`](EngineRequest::WriteInput) path — `ExternalIn`
//! observes the pin change and resubscribes automatically.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::base::block::BlockState;
use crate::base::error::RequestError;
use crate::base::program::{EditorMetadata, Position, Program, data::LinkData};

/// Block input properties.
///
/// `#[non_exhaustive]`: engines produce it; build one with
/// [`BlockInputData::new`].
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BlockInputData {
    /// Haystack type kind.
    pub kind: String,
    /// Current value.
    pub val: Value,
    /// True if this input is wired up (i.e. its `connection_count > 0`).
    /// Surfaced so `save_program` can round-trip pin connectedness
    /// alongside the value.
    #[serde(default)]
    pub is_connected: bool,
}

impl BlockInputData {
    /// An input of `kind` holding `val`, wired up or not.
    pub fn new(kind: impl Into<String>, val: Value, is_connected: bool) -> Self {
        Self {
            kind: kind.into(),
            val,
            is_connected,
        }
    }
}

/// Block output properties.
///
/// `#[non_exhaustive]`: engines produce it; build one with
/// [`BlockOutputData::new`].
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct BlockOutputData {
    /// Haystack type kind.
    pub kind: String,
    /// Current value.
    pub val: Value,
}

impl BlockOutputData {
    /// An output of `kind` holding `val`.
    pub fn new(kind: impl Into<String>, val: Value) -> Self {
        Self {
            kind: kind.into(),
            val,
        }
    }
}

/// A block's state and pin values, as
/// [`EngineRequest::InspectBlock`] answers with them.
///
/// `#[non_exhaustive]`: engines produce it; build one with
/// [`BlockDefinition::new`] and the `with_*` methods.
///
/// Its fields are typed like a [`WatchMessage`]'s. Serialized, it is a
/// flat object with camelCase names, like the pins it holds: `id` is
/// the block UUID as a string, `state` the state's
/// [label](BlockState::label), and `faultReason` — left out unless the
/// block is in Fault — the fault's reason.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     Uuid, Value,
///     base::block::BlockState,
///     base::engine::messages::{BlockDefinition, BlockInputData},
/// };
///
/// let id = Uuid::new_v4();
/// let def = BlockDefinition::new(id, "Add", "core")
///     .with_input("in0", BlockInputData::new("number", Value::make_int(1), false))
///     .with_state(BlockState::fault("overflow"));
///
/// assert_eq!(def.id, id);
/// assert_eq!(def.state, BlockState::fault("overflow"));
/// assert_eq!(def.inputs["in0"].val, Value::make_int(1));
///
/// let json = serde_json::to_value(&def)?;
/// assert_eq!(json["id"], id.to_string());
/// assert_eq!(json["state"], "fault");
/// assert_eq!(json["faultReason"], "overflow");
/// assert_eq!(json["inputs"]["in0"]["isConnected"], false);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(into = "WireBlockDefinition", try_from = "WireBlockDefinition")]
#[non_exhaustive]
pub struct BlockDefinition {
    /// Block instance UUID.
    pub id: Uuid,
    /// Block type name.
    pub name: String,
    /// Library the block belongs to.
    pub library: String,
    /// Input pins keyed by name.
    pub inputs: BTreeMap<String, BlockInputData>,
    /// Output pins keyed by name.
    pub outputs: BTreeMap<String, BlockOutputData>,
    /// The block's operational state, with the fault's reason when it
    /// is in Fault.
    pub state: BlockState,
}

impl BlockDefinition {
    /// The definition of block `id` of type `name` in `library`, with
    /// no pins, in the [`BlockState::Running`] state.
    pub fn new(id: Uuid, name: impl Into<String>, library: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            library: library.into(),
            ..Self::default()
        }
    }

    /// Adds input `name`, replacing an input of that name.
    pub fn with_input(mut self, name: impl Into<String>, input: BlockInputData) -> Self {
        self.inputs.insert(name.into(), input);
        self
    }

    /// Adds output `name`, replacing an output of that name.
    pub fn with_output(mut self, name: impl Into<String>, output: BlockOutputData) -> Self {
        self.outputs.insert(name.into(), output);
        self
    }

    /// Sets [`state`](Self::state).
    pub fn with_state(mut self, state: BlockState) -> Self {
        self.state = state;
        self
    }
}

/// The serialized form of a [`BlockDefinition`].
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireBlockDefinition {
    id: String,
    name: String,
    library: String,
    #[serde(default)]
    inputs: BTreeMap<String, BlockInputData>,
    #[serde(default)]
    outputs: BTreeMap<String, BlockOutputData>,
    #[serde(default = "running_label")]
    state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fault_reason: Option<String>,
}

/// The label of [`BlockState::Running`], the state of a serialized
/// [`BlockDefinition`] that gives none.
fn running_label() -> String {
    BlockState::Running.label().to_string()
}

/// Why a serialized [`BlockDefinition`] could not be read.
#[derive(Debug, thiserror::Error)]
enum WireBlockDefinitionError {
    /// Its `id` is not a UUID.
    #[error("Invalid block uuid '{id}'")]
    InvalidId {
        /// The rejected id.
        id: String,
        /// The parse error.
        #[source]
        source: uuid::Error,
    },
    /// Its `state` is not the label of a [`BlockState`].
    #[error("Unknown block state '{0}'")]
    UnknownState(String),
}

impl From<BlockDefinition> for WireBlockDefinition {
    fn from(def: BlockDefinition) -> Self {
        Self {
            id: def.id.to_string(),
            name: def.name,
            library: def.library,
            inputs: def.inputs,
            outputs: def.outputs,
            state: def.state.label().to_string(),
            fault_reason: def.state.fault_reason().map(str::to_string),
        }
    }
}

impl TryFrom<WireBlockDefinition> for BlockDefinition {
    type Error = WireBlockDefinitionError;

    fn try_from(wire: WireBlockDefinition) -> Result<Self, Self::Error> {
        let id =
            Uuid::try_parse(&wire.id).map_err(|source| WireBlockDefinitionError::InvalidId {
                id: wire.id.clone(),
                source,
            })?;
        let state = match wire.state.as_str() {
            "running" => BlockState::Running,
            "fault" => BlockState::fault(wire.fault_reason.unwrap_or_default()),
            "disabled" => BlockState::Disabled,
            "terminated" => BlockState::Terminated,
            _ => return Err(WireBlockDefinitionError::UnknownState(wire.state)),
        };
        Ok(Self {
            id,
            name: wire.name,
            library: wire.library,
            inputs: wire.inputs,
            outputs: wire.outputs,
            state,
        })
    }
}

/// The source of a change.
///
/// `#[non_exhaustive]`: a `match` on it needs a wildcard arm.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ChangeSource {
    /// Change originated from an input pin.
    Input(String, Value),
    /// Change originated from an output pin.
    Output(String, Value),
}

/// A notification message for a block change.
///
/// `#[non_exhaustive]`: engines produce it; build one with
/// [`WatchMessage::new`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct WatchMessage {
    /// UUID of the block that changed.
    pub block_id: Uuid,
    /// Changed pins keyed by name.
    pub changes: HashMap<String, ChangeSource>,
    /// Block's operational state at the time the notification was sent.
    /// Carries fault propagation visibility to the UI.
    pub state: BlockState,
}

impl WatchMessage {
    /// A notification that block `block_id`, in `state`, changed the
    /// pins in `changes`.
    pub fn new(block_id: Uuid, changes: HashMap<String, ChangeSource>, state: BlockState) -> Self {
        Self {
            block_id,
            changes,
            state,
        }
    }
}

/// Identifies a client of an engine: the response channel the engine
/// answers its requests on.
///
/// The engine assigns it when the channel is opened
/// ([`Engine::create_message_channel`](crate::base::engine::Engine::create_message_channel)),
/// unique among the engine's clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(u64);

impl ClientId {
    /// The client id numbered `id`.
    pub(crate) fn new(id: u64) -> Self {
        Self(id)
    }

    /// The id as a number.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client#{}", self.0)
    }
}

/// Identifies a request among its client's requests; its response
/// carries the same id.
///
/// The client chooses it. An engine echoes it back without reading it,
/// so ids only need to be unique among a client's requests in flight —
/// [`EngineClient`](crate::base::engine::EngineClient) numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request#{}", self.0)
    }
}

/// Identifies a watch: a subscription to block change notifications,
/// which [`EngineRequest::Watch`] opens and [`EngineRequest::Unwatch`]
/// closes.
///
/// The engine assigns it, unique among the engine's watches. Watch ids
/// are engine-wide, not scoped to the client that opened the watch: any
/// client may close any watch, as any client may remove any block.
///
/// A transport that carries a watch id across as a number — its
/// [`get`](Self::get) — reads it back with [`new`](Self::new).
///
/// # Examples
///
/// ```
/// use logic_mesh::base::engine::messages::WatchId;
///
/// let id = WatchId::new(3);
/// assert_eq!(WatchId::new(id.get()), id);
/// assert_eq!(id.to_string(), "watch#3");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WatchId(u64);

impl WatchId {
    /// The watch id numbered `id`.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// The id as a number.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "watch#{}", self.0)
    }
}

/// A message between an engine and one of its clients: a request or a
/// response, with the ids that route and correlate it.
///
/// `#[non_exhaustive]`: build one with [`Envelope::new`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Envelope<T> {
    /// The client the request came from, and the response goes to.
    pub client: ClientId,
    /// The request's id, which its response carries back.
    pub id: RequestId,
    /// The request or response.
    pub body: T,
}

impl<T> Envelope<T> {
    /// The message `body` of request `id` from `client`.
    pub fn new(client: ClientId, id: RequestId, body: T) -> Self {
        Self { client, id, body }
    }
}

/// A request on its way to an engine.
pub type RequestMessage = Envelope<EngineRequest>;

/// A response on its way back to a client.
pub type ResponseMessage = Envelope<EngineResponse>;

/// The channel an engine receives requests on. Every client of the
/// engine sends on a clone of it.
pub type RequestSender = mpsc::Sender<RequestMessage>;

/// The channel an engine answers one client's requests on.
///
/// Unbounded, so the engine never waits on a slow client and never
/// drops a response: each response answers a request the client sent,
/// so the client's own requests bound what it has to read.
pub type ResponseSender = mpsc::UnboundedSender<ResponseMessage>;

/// The channel a watch delivers block change notifications on.
///
/// Unbounded: fault notifications must not drop under a burst of
/// changes, such as dozens of blocks faulting as a program loads.
pub type WatchSender = mpsc::UnboundedSender<WatchMessage>;

/// A request an engine answers.
///
/// Each is answered with the [`EngineResponse`] variant of the same
/// name. A running engine handles requests one at a time, in the order
/// it receives them, and answers each before it handles the next — but a
/// client matches responses to requests by [`RequestId`], not by order.
/// An idle engine queues the requests sent to it, and handles them once
/// it runs.
///
/// `#[non_exhaustive]`: new requests may be added in minor releases.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EngineRequest {
    /// Adds a block of type `name` and schedules it. Without `library`,
    /// every library is searched, and a name more than one library holds
    /// is rejected. A `block_id` that is already scheduled is rejected
    /// rather than replacing the scheduled block. Answered with the new
    /// block's id.
    AddBlock {
        /// The block type name.
        name: String,
        /// The id to give the block; a fresh one if `None`.
        block_id: Option<Uuid>,
        /// The library of the block type.
        library: Option<String>,
    },

    /// Removes a block with all its links — also when its actor task is
    /// gone already. Answered with the removed block's id.
    RemoveBlock {
        /// The block to remove.
        block: Uuid,
    },

    /// Links two block pins. The link gets the id [`LinkData::id`] names,
    /// if set, and a fresh one otherwise. Link ids are unique per engine:
    /// an id another link already has is rejected, as is one that is not
    /// a UUID. Answered with the link, with its id.
    Connect {
        /// The link to make.
        link: LinkData,
    },

    /// Removes a link by id. Answered with whether a link was removed.
    Disconnect {
        /// The link to remove.
        link: Uuid,
    },

    /// Writes a value to a block's input pin, which runs the block,
    /// linked or not. Answered with the input's previous value.
    WriteInput {
        /// The block.
        block: Uuid,
        /// The input pin name.
        pin: String,
        /// The value to write.
        value: Value,
    },

    /// Writes a value to a block's output pin, which reaches the inputs
    /// linked to it. Answered with the output's previous value, as
    /// [`WriteInput`](Self::WriteInput) is with the input's.
    WriteOutput {
        /// The block.
        block: Uuid,
        /// The output pin name.
        pin: String,
        /// The value to write.
        value: Value,
    },

    /// Reads a block's current state and pin values.
    InspectBlock {
        /// The block to inspect.
        block: Uuid,
    },

    /// Evaluates a block type once, outside the engine's graph: a
    /// temporary block of type `name` gets `inputs` and executes, and
    /// its output values are the answer. Without `library`, every
    /// library is searched, as for [`AddBlock`](Self::AddBlock).
    ///
    /// The block runs on the engine's event loop, which answers no
    /// other request until it is done, so the evaluation is guarded: a
    /// panic in the block fails it with
    /// [`EngineError::EvalPanicked`](crate::EngineError::EvalPanicked)
    /// (on native targets; a panic aborts a `wasm32` instance), and one
    /// still running after 5 s — a block waiting for an input that
    /// `inputs` does not supply, say — is dropped and fails with
    /// [`EngineError::EvalTimeout`](crate::EngineError::EvalTimeout).
    EvalBlock {
        /// The block type name.
        name: String,
        /// The input values, in the block's input order.
        inputs: Vec<Value>,
        /// The library of the block type.
        library: Option<String>,
    },

    /// Answered with the current program in save format: the name and
    /// description of the program loaded last (none after a
    /// [`Reset`](Self::Reset)), and every block and link the engine
    /// holds.
    ///
    /// A block whose actor task is gone — its pin values with it — is
    /// saved with its type, UI metadata and links but no pin values,
    /// rather than failing the whole save.
    SaveProgram,

    /// Loads a full [`Program`] (blocks, links, pin values, UI metadata,
    /// name and description) into the engine.
    ///
    /// All or nothing: the program is checked whole before any of its
    /// blocks is scheduled — the checks of
    /// [`Engine::schedule_program_blocks`](crate::base::engine::Engine::schedule_program_blocks),
    /// so a block or link id the engine holds already is rejected — and
    /// a failure while wiring it removes its blocks again. A failed load
    /// leaves the engine as it was, though blocks that ran meanwhile may
    /// have had effects outside it, such as a connector publish.
    ///
    /// Every link keeps the id it was saved under — its
    /// [`LinkData::id`], or else its key in [`Program::links`] — so a
    /// [`Disconnect`](Self::Disconnect) can name a saved id.
    LoadProgram {
        /// The program to load.
        program: Program,
    },

    /// Updates the label and position a block is saved with: each one
    /// given replaces the stored one. The running-phase counterpart of
    /// [`Engine::set_block_metadata`](crate::base::engine::Engine::set_block_metadata).
    SetBlockMetadata {
        /// The block.
        block: Uuid,
        /// The new label, if it changes.
        label: Option<String>,
        /// The new position, if it changes.
        position: Option<Position>,
    },

    /// Replaces the editor metadata a block is saved with, or removes it
    /// if `editor` is `None`. The running-phase counterpart of
    /// [`Engine::set_block_editor`](crate::base::engine::Engine::set_block_editor).
    SetBlockEditor {
        /// The block.
        block: Uuid,
        /// The new editor metadata.
        editor: Option<EditorMetadata>,
    },

    /// Opens a watch: from now on, every block change is sent to
    /// `sender` as a [`WatchMessage`], until an [`Unwatch`](Self::Unwatch)
    /// closes the watch or `sender`'s receiver is dropped. Answered with
    /// the watch's id.
    Watch {
        /// Where to send the notifications.
        sender: WatchSender,
    },

    /// Closes a watch, dropping its sender. Answered with whether the
    /// watch was open.
    ///
    /// Any client may close any watch: [`WatchId`]s are engine-wide. A
    /// transport serving peers that must not close each other's watches
    /// keeps its own record of which peer opened which watch.
    Unwatch {
        /// The watch to close.
        watch: WatchId,
    },

    /// Attaches an already-registered connector, by name: the engine
    /// resolves the name in the process-wide connector registry, starts
    /// the connector, and manages its lifecycle from then on. Answered
    /// with the connector's name.
    ///
    /// Connector handles never ride in requests (they are not `Send`
    /// on every target) — register the handle through
    /// [`register_connector`](crate::base::connector::register_connector)
    /// first, then attach it by name.
    ///
    /// A registered connector must be attached to at most one engine:
    /// the registry carries no ownership mark, so attaching the same
    /// name to a second engine would double-start the connector, and
    /// one engine's reset would unregister it out from under the
    /// other.
    AddConnector {
        /// The registered connector's name.
        name: String,
    },

    /// Detaches an engine-managed connector by name: the engine stops
    /// tracking it, unregisters it from the process-wide registry, and
    /// then stops it — same ordering as a reset, so no block can
    /// resolve a connector that is being stopped. Answered with the
    /// connector's name.
    RemoveConnector {
        /// The engine-managed connector's name.
        name: String,
    },

    /// Answered with the names of the engine-managed connectors.
    ListConnectors,

    /// Pauses block execution. Answered once no block starts executing
    /// any more. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-requests).
    Pause,

    /// Resumes block execution. Answered once blocks may execute again.
    /// See the [lifecycle contract](crate::base::engine#lifecycle-requests).
    Resume,

    /// Resets the engine: removes all blocks and links, forgets the
    /// loaded program's name and description, and unregisters and stops
    /// the managed connectors, paused or not. Answered once that is
    /// done. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-requests).
    Reset,

    /// Shuts the engine down: stops block execution and the managed
    /// connectors, answers, and ends
    /// [`run`](crate::base::engine::Engine::run), which hands back the
    /// idle engine with its blocks intact. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-requests).
    Shutdown,
}

impl EngineRequest {
    /// The request's variant name, such as `"AddBlock"` — what to log
    /// about it without its data.
    pub fn name(&self) -> &'static str {
        match self {
            EngineRequest::AddBlock { .. } => "AddBlock",
            EngineRequest::RemoveBlock { .. } => "RemoveBlock",
            EngineRequest::Connect { .. } => "Connect",
            EngineRequest::Disconnect { .. } => "Disconnect",
            EngineRequest::WriteInput { .. } => "WriteInput",
            EngineRequest::WriteOutput { .. } => "WriteOutput",
            EngineRequest::InspectBlock { .. } => "InspectBlock",
            EngineRequest::EvalBlock { .. } => "EvalBlock",
            EngineRequest::SaveProgram => "SaveProgram",
            EngineRequest::LoadProgram { .. } => "LoadProgram",
            EngineRequest::SetBlockMetadata { .. } => "SetBlockMetadata",
            EngineRequest::SetBlockEditor { .. } => "SetBlockEditor",
            EngineRequest::Watch { .. } => "Watch",
            EngineRequest::Unwatch { .. } => "Unwatch",
            EngineRequest::AddConnector { .. } => "AddConnector",
            EngineRequest::RemoveConnector { .. } => "RemoveConnector",
            EngineRequest::ListConnectors => "ListConnectors",
            EngineRequest::Pause => "Pause",
            EngineRequest::Resume => "Resume",
            EngineRequest::Reset => "Reset",
            EngineRequest::Shutdown => "Shutdown",
        }
    }
}

/// An engine's answer to an [`EngineRequest`]: the variant of the same
/// name, carrying the request's `result`.
///
/// `#[non_exhaustive]`: new responses may be added in minor releases.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EngineResponse {
    /// Answers [`EngineRequest::AddBlock`].
    AddBlock {
        /// The new block's id.
        result: Result<Uuid, RequestError>,
    },
    /// Answers [`EngineRequest::RemoveBlock`].
    RemoveBlock {
        /// The removed block's id.
        result: Result<Uuid, RequestError>,
    },
    /// Answers [`EngineRequest::Connect`].
    Connect {
        /// The link, with its id.
        result: Result<LinkData, RequestError>,
    },
    /// Answers [`EngineRequest::Disconnect`].
    Disconnect {
        /// Whether a link was removed.
        result: Result<bool, RequestError>,
    },
    /// Answers [`EngineRequest::WriteInput`].
    WriteInput {
        /// The input's previous value.
        result: Result<Option<Value>, RequestError>,
    },
    /// Answers [`EngineRequest::WriteOutput`].
    WriteOutput {
        /// The output's previous value.
        result: Result<Value, RequestError>,
    },
    /// Answers [`EngineRequest::InspectBlock`].
    InspectBlock {
        /// The block's state and pin values.
        result: Result<BlockDefinition, RequestError>,
    },
    /// Answers [`EngineRequest::EvalBlock`].
    EvalBlock {
        /// The block's output values.
        result: Result<Vec<Value>, RequestError>,
    },
    /// Answers [`EngineRequest::SaveProgram`].
    SaveProgram {
        /// The current program.
        result: Result<Program, RequestError>,
    },
    /// Answers [`EngineRequest::LoadProgram`].
    LoadProgram {
        /// Whether the program loaded.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::SetBlockMetadata`].
    SetBlockMetadata {
        /// Whether the metadata was set.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::SetBlockEditor`].
    SetBlockEditor {
        /// Whether the editor metadata was set.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::Watch`].
    Watch {
        /// The new watch's id.
        result: Result<WatchId, RequestError>,
    },
    /// Answers [`EngineRequest::Unwatch`].
    Unwatch {
        /// Whether the watch was open.
        result: Result<bool, RequestError>,
    },
    /// Answers [`EngineRequest::AddConnector`].
    AddConnector {
        /// The attached connector's name.
        result: Result<String, RequestError>,
    },
    /// Answers [`EngineRequest::RemoveConnector`].
    RemoveConnector {
        /// The detached connector's name.
        result: Result<String, RequestError>,
    },
    /// Answers [`EngineRequest::ListConnectors`].
    ListConnectors {
        /// The names of the engine-managed connectors.
        result: Result<Vec<String>, RequestError>,
    },
    /// Answers [`EngineRequest::Pause`] once block execution stopped.
    Pause {
        /// The acknowledgement.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::Resume`] once block execution restarted.
    Resume {
        /// The acknowledgement.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::Reset`] once the engine was reset.
    Reset {
        /// The acknowledgement.
        result: Result<(), RequestError>,
    },
    /// Answers [`EngineRequest::Shutdown`] once block execution and the
    /// managed connectors stopped, as `run` returns.
    Shutdown {
        /// The acknowledgement.
        result: Result<(), RequestError>,
    },
}

impl EngineResponse {
    /// The response's variant name, such as `"AddBlock"` — what to log
    /// about it without its data, which can be a whole program.
    pub fn name(&self) -> &'static str {
        match self {
            EngineResponse::AddBlock { .. } => "AddBlock",
            EngineResponse::RemoveBlock { .. } => "RemoveBlock",
            EngineResponse::Connect { .. } => "Connect",
            EngineResponse::Disconnect { .. } => "Disconnect",
            EngineResponse::WriteInput { .. } => "WriteInput",
            EngineResponse::WriteOutput { .. } => "WriteOutput",
            EngineResponse::InspectBlock { .. } => "InspectBlock",
            EngineResponse::EvalBlock { .. } => "EvalBlock",
            EngineResponse::SaveProgram { .. } => "SaveProgram",
            EngineResponse::LoadProgram { .. } => "LoadProgram",
            EngineResponse::SetBlockMetadata { .. } => "SetBlockMetadata",
            EngineResponse::SetBlockEditor { .. } => "SetBlockEditor",
            EngineResponse::Watch { .. } => "Watch",
            EngineResponse::Unwatch { .. } => "Unwatch",
            EngineResponse::AddConnector { .. } => "AddConnector",
            EngineResponse::RemoveConnector { .. } => "RemoveConnector",
            EngineResponse::ListConnectors { .. } => "ListConnectors",
            EngineResponse::Pause { .. } => "Pause",
            EngineResponse::Resume { .. } => "Resume",
            EngineResponse::Reset { .. } => "Reset",
            EngineResponse::Shutdown { .. } => "Shutdown",
        }
    }

    /// The error the request failed with, if it failed.
    pub fn error(&self) -> Option<&RequestError> {
        match self {
            EngineResponse::AddBlock { result } | EngineResponse::RemoveBlock { result } => {
                result.as_ref().err()
            }
            EngineResponse::Connect { result } => result.as_ref().err(),
            EngineResponse::Disconnect { result } | EngineResponse::Unwatch { result } => {
                result.as_ref().err()
            }
            EngineResponse::WriteInput { result } => result.as_ref().err(),
            EngineResponse::WriteOutput { result } => result.as_ref().err(),
            EngineResponse::InspectBlock { result } => result.as_ref().err(),
            EngineResponse::EvalBlock { result } => result.as_ref().err(),
            EngineResponse::SaveProgram { result } => result.as_ref().err(),
            EngineResponse::Watch { result } => result.as_ref().err(),
            EngineResponse::AddConnector { result }
            | EngineResponse::RemoveConnector { result } => result.as_ref().err(),
            EngineResponse::ListConnectors { result } => result.as_ref().err(),
            EngineResponse::LoadProgram { result }
            | EngineResponse::SetBlockMetadata { result }
            | EngineResponse::SetBlockEditor { result }
            | EngineResponse::Pause { result }
            | EngineResponse::Resume { result }
            | EngineResponse::Reset { result }
            | EngineResponse::Shutdown { result } => result.as_ref().err(),
        }
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::Value;
    use serde_json::json;
    use uuid::Uuid;

    use super::{BlockDefinition, BlockInputData, BlockOutputData};
    use crate::base::block::BlockState;

    /// Regression: a block definition held its id and state as strings,
    /// with the fault reason beside them, and serialized `fault_reason`
    /// in snake_case next to its pins' camelCase `isConnected`.
    #[test]
    fn a_block_definition_is_typed_and_serializes_in_camel_case() {
        let id = Uuid::new_v4();
        let def = BlockDefinition::new(id, "Add", "core")
            .with_input(
                "in0",
                BlockInputData::new("number", Value::make_int(1), true),
            )
            .with_output("out", BlockOutputData::new("number", Value::make_int(2)))
            .with_state(BlockState::fault("overflow"));

        let written = serde_json::to_value(&def).expect("serializes");
        assert_eq!(
            written,
            json!({
                "id": id.to_string(),
                "name": "Add",
                "library": "core",
                "inputs": {"in0": {"kind": "number", "val": 1, "isConnected": true}},
                "outputs": {"out": {"kind": "number", "val": 2}},
                "state": "fault",
                "faultReason": "overflow",
            })
        );

        let read: BlockDefinition = serde_json::from_value(written).expect("reads back");
        assert_eq!(read.id, id);
        assert_eq!(read.state, BlockState::fault("overflow"));
        assert_eq!(read.inputs["in0"].val, Value::make_int(1));
        assert_eq!(read.outputs["out"].val, Value::make_int(2));
    }

    /// Every state reads back as itself, and only a fault carries a
    /// reason.
    #[test]
    fn every_block_state_round_trips() {
        for state in [
            BlockState::Running,
            BlockState::fault("why"),
            BlockState::Disabled,
            BlockState::Terminated,
        ] {
            let def = BlockDefinition::new(Uuid::nil(), "Add", "core").with_state(state.clone());
            let written = serde_json::to_value(&def).expect("serializes");
            assert_eq!(written["state"], state.label());
            assert_eq!(written.get("faultReason").is_some(), state.is_fault());
            let read: BlockDefinition = serde_json::from_value(written).expect("reads back");
            assert_eq!(read.state, state);
        }
    }

    /// A definition whose id is not a UUID, or whose state no
    /// `BlockState` has, is rejected rather than read as a default.
    #[test]
    fn a_malformed_block_definition_is_rejected() {
        let def = |id: &str, state: &str| json!({"id": id, "name": "Add", "library": "core", "state": state});
        let ok =
            serde_json::from_value::<BlockDefinition>(def(&Uuid::nil().to_string(), "running"));
        assert_eq!(ok.expect("reads").state, BlockState::Running);

        let err = serde_json::from_value::<BlockDefinition>(def("b1", "running"))
            .expect_err("not a uuid");
        assert!(err.to_string().contains("Invalid block uuid 'b1'"), "{err}");
        let err =
            serde_json::from_value::<BlockDefinition>(def(&Uuid::nil().to_string(), "asleep"))
                .expect_err("not a state");
        assert!(
            err.to_string().contains("Unknown block state 'asleep'"),
            "{err}"
        );
    }
}
