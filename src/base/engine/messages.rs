// Copyright (c) 2022-2023, Radu Racariu.

//! Engine message types.
//!
//! # Connector routing
//!
//! Connector route/subscription management needs no dedicated
//! messages: rebinding an external block is a write to its
//! `connector` / `address` input pins through the existing
//! [`WriteBlockInputReq`](EngineMessage::WriteBlockInputReq) path —
//! `ExternalIn` observes the pin change and resubscribes
//! automatically.

use std::collections::{BTreeMap, HashMap};

use libhaystack::val::Value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::base::block::BlockState;
use crate::base::program::{Program, data::LinkData};

/// Block input properties.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
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

/// Block output properties.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BlockOutputData {
    /// Haystack type kind.
    pub kind: String,
    /// Current value.
    pub val: Value,
}

/// Block definition.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BlockDefinition {
    /// Block instance UUID.
    pub id: String,
    /// Block type name.
    pub name: String,
    /// Library the block belongs to.
    pub library: String,
    /// Input pins keyed by name.
    pub inputs: BTreeMap<String, BlockInputData>,
    /// Output pins keyed by name.
    pub outputs: BTreeMap<String, BlockOutputData>,
    /// Short label for the block's operational state
    /// (`running | fault | disabled | terminated`).
    #[serde(default)]
    pub state: String,
    /// Fault reason when `state == "fault"`, else [`None`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fault_reason: Option<String>,
}

/// The source of a change.
#[derive(Debug, Clone)]
pub enum ChangeSource {
    /// Change originated from an input pin.
    Input(String, Value),
    /// Change originated from an output pin.
    Output(String, Value),
}

/// A notification message for a block change.
#[derive(Debug, Clone)]
pub struct WatchMessage {
    /// UUID of the block that changed.
    pub block_id: Uuid,
    /// Changed pins keyed by name.
    pub changes: HashMap<String, ChangeSource>,
    /// Block's operational state at the time the notification was sent.
    /// Carries fault propagation visibility to the UI.
    pub state: BlockState,
}

/// Messages that the engine accepts.
#[derive(Debug, Clone)]
pub enum EngineMessage<WatchEventSender: Clone> {
    /// Request to add a block by name. A caller-supplied block id that
    /// is already scheduled is rejected rather than replacing the
    /// scheduled block.
    AddBlockReq(Uuid, String, Option<String>, Option<String>),
    /// Response to [`AddBlockReq`](Self::AddBlockReq).
    AddBlockRes(Result<Uuid, String>),

    /// Request to remove a block by UUID, with all its links — also
    /// when its actor task is gone already.
    RemoveBlockReq(Uuid, Uuid),
    /// Response to [`RemoveBlockReq`](Self::RemoveBlockReq).
    RemoveBlockRes(Result<Uuid, String>),

    /// Subscribe to block change-of-value notifications.
    WatchBlockSubReq(Uuid, WatchEventSender),
    /// Response to [`WatchBlockSubReq`](Self::WatchBlockSubReq).
    WatchBlockSubRes(Result<Uuid, &'static str>),

    /// Writes a value to a block's output pin.
    WriteBlockOutputReq(Uuid, Uuid, String, Value),
    /// Response to [`WriteBlockOutputReq`](Self::WriteBlockOutputReq).
    WriteBlockOutputRes(Result<Value, String>),

    /// Writes a value to a block's input pin.
    WriteBlockInputReq(Uuid, Uuid, String, Value),
    /// Response to [`WriteBlockInputReq`](Self::WriteBlockInputReq).
    WriteBlockInputRes(Result<Option<Value>, String>),

    /// Unsubscribe from block change notifications.
    WatchBlockUnsubReq(Uuid),
    /// Response to [`WatchBlockUnsubReq`](Self::WatchBlockUnsubReq).
    WatchBlockUnsubRes(Result<Uuid, &'static str>),

    /// Request the current program in save format: the name and
    /// description of the program loaded last (none after a
    /// [`Reset`](Self::Reset)), and every block and link the engine
    /// holds.
    ///
    /// A block whose actor task is gone — its pin values with it — is
    /// saved with its type, UI metadata and links but no pin values,
    /// rather than failing the whole save.
    GetCurrentProgramReq(Uuid),
    /// Response to [`GetCurrentProgramReq`](Self::GetCurrentProgramReq).
    GetCurrentProgramRes(Result<Program, String>),

    /// Atomically load a full [`Program`] (blocks, links, pin values, UI
    /// metadata, name and description) into the engine. Replaces the
    /// multi-call JS chain of `addBlock` + `createLink` +
    /// `writeBlockInput` per block.
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
    /// [`RemoveLinkReq`](Self::RemoveLinkReq) can name a saved id.
    LoadProgramReq(Uuid, Program),
    /// Response to [`LoadProgramReq`](Self::LoadProgramReq).
    LoadProgramRes(Result<(), String>),

    /// Request to inspect a block's current state.
    InspectBlockReq(Uuid, Uuid),
    /// Response to [`InspectBlockReq`](Self::InspectBlockReq).
    InspectBlockRes(Result<BlockDefinition, String>),

    /// Evaluates a block by name with given inputs.
    EvaluateBlockReq(Uuid, String, Vec<Value>, Option<String>),
    /// Response to [`EvaluateBlockReq`](Self::EvaluateBlockReq).
    EvaluateBlockRes(Result<Vec<Value>, String>),

    /// Connects two blocks via a link.
    ///
    /// The link gets the id [`LinkData::id`] names, if set, and a fresh
    /// one otherwise. Link ids are unique per engine: an id another link
    /// already has is rejected, as is one that is not a UUID.
    ConnectBlocksReq(Uuid, LinkData),
    /// Response to [`ConnectBlocksReq`](Self::ConnectBlocksReq): the link,
    /// with its id.
    ConnectBlocksRes(Result<LinkData, String>),

    /// Removes a link by UUID.
    RemoveLinkReq(Uuid, Uuid),
    /// Response to [`RemoveLinkReq`](Self::RemoveLinkReq).
    RemoveLinkRes(Result<bool, String>),

    /// Attach an already-registered connector, by name, to a running
    /// engine: the engine resolves the name in the process-wide
    /// connector registry, starts the connector, and manages its
    /// lifecycle from then on.
    ///
    /// Connector handles never ride in messages (they are not `Send`
    /// on every target) — register the handle through
    /// [`register_connector`](crate::base::connector::register_connector)
    /// first, then attach it by name.
    ///
    /// A registered connector must be attached to at most one engine:
    /// the registry carries no ownership mark, so attaching the same
    /// name to a second engine would double-start the connector, and
    /// one engine's reset would unregister it out from under the
    /// other.
    AddConnectorReq(Uuid, String),
    /// Response to [`AddConnectorReq`](Self::AddConnectorReq). `Ok`
    /// carries the attached connector's name; errors are: not
    /// registered, already engine-managed, or start failure/timeout.
    AddConnectorRes(Result<String, String>),

    /// Detach an engine-managed connector by name: the engine stops
    /// tracking it, unregisters it from the process-wide registry, and
    /// then stops it — same ordering as a reset, so no block can
    /// resolve a connector that is being stopped.
    RemoveConnectorReq(Uuid, String),
    /// Response to [`RemoveConnectorReq`](Self::RemoveConnectorReq).
    /// `Ok` carries the detached connector's name; errors if the name
    /// is not engine-managed.
    RemoveConnectorRes(Result<String, String>),

    /// Request the names of the engine-managed connectors.
    ListConnectorsReq(Uuid),
    /// Response to [`ListConnectorsReq`](Self::ListConnectorsReq).
    ListConnectorsRes(Result<Vec<String>, String>),

    /// Shut down the engine: stop block execution and the managed
    /// connectors and end [`run`](crate::base::engine::Engine::run),
    /// which hands back the idle engine with its blocks intact. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    Shutdown,
    /// Pause block execution. Requests are still answered. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    Pause,
    /// Resume block execution. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    Resume,
    /// Reset the engine, removing all blocks and links and forgetting
    /// the loaded program's name and description, paused or not. See the
    /// [lifecycle contract](crate::base::engine#lifecycle-messages).
    Reset,
}
