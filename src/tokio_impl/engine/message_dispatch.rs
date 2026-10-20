// Copyright (c) 2022-2026, Radu Racariu.

//! Engine-request dispatcher, shared by both engines.
//!
//! Each [`EngineRequest`] arriving on a running engine's request
//! channel is carried out against its block graph — most as one or more
//! per-block mailbox round-trips — and answered with exactly one
//! [`EngineResponse`], on the response channel of the client that sent
//! it. The engines differ only in what [`RunningHost`] abstracts: how
//! they store watchers and gate block execution.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::base::engine::messages::{
    ClientId, EngineRequest, EngineResponse, Envelope, RequestId, RequestMessage, ResponseSender,
    WatchId, WatchSender,
};
use crate::base::error::{EngineError, Result};
use crate::blocks::registry::get_block;
use crate::tokio_impl::sleep::sleep_millis;

use super::actor_step::catch_panic;
use super::connectors::{self, ManagedConnectors};
use super::eval_block;
use super::graph::{ProgramHost, stage_program};

/// The clients of an engine: the response channel of each, by the id
/// the engine assigned it, and the counters the engine assigns client
/// and watch ids from.
#[derive(Default)]
pub(super) struct Clients {
    /// Every client's response channel.
    responses: BTreeMap<ClientId, ResponseSender>,
    /// The id the next client gets.
    next_client: u64,
    /// The id the next watch gets.
    next_watch: u64,
}

impl Clients {
    /// Records a client answered on `responses`, returning its new id.
    pub(super) fn register(&mut self, responses: ResponseSender) -> ClientId {
        let id = ClientId::new(self.next_client);
        self.next_client += 1;
        self.responses.insert(id, responses);
        id
    }

    /// A new watch id.
    pub(super) fn next_watch(&mut self) -> WatchId {
        let id = WatchId::new(self.next_watch);
        self.next_watch += 1;
        id
    }

    /// Sends `response` to request `id` of `client`.
    ///
    /// The channel is unbounded, so this never waits on the client, and
    /// fails only once the client dropped its receiver: it is not
    /// waiting for the response any more, so it is dropped. The client
    /// stays registered — a raw caller may drop its receiver and keep
    /// sending requests it does not need answered.
    fn respond(&self, client: ClientId, id: RequestId, response: EngineResponse) {
        let name = response.name();
        let Some(sender) = self.responses.get(&client) else {
            log::warn!("Dropping the {name} response to {id}: {client} is unknown");
            return;
        };
        if sender.send(Envelope::new(client, id, response)).is_err() {
            log::debug!("Dropping the {name} response to {id}: {client} stopped listening");
        }
    }
}

/// What a running engine provides the dispatcher beyond its block graph.
pub(super) trait RunningHost: ProgramHost {
    /// The engine's clients.
    fn clients(&mut self) -> &mut Clients;

    /// The names of the connectors the engine manages.
    fn connectors(&mut self) -> &mut ManagedConnectors;

    /// Opens or closes the execution gate for every block.
    fn set_execution(&self, open: bool);

    /// Starts sending block change notifications to `sender`, as watch
    /// `id`.
    async fn add_watcher(&mut self, id: WatchId, sender: WatchSender);

    /// Stops notifying watch `id`, returning whether it was open.
    async fn remove_watcher(&mut self, id: WatchId) -> bool;
}

/// Whether the event loop goes on after a request.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Flow {
    /// Wait for the next request.
    Continue,
    /// The engine shut down: end `run()`.
    Shutdown,
}

/// Carries out `message` on `engine` and answers it.
///
/// Every request is answered exactly once, failed or not, and nothing
/// here panics: a failure is the response's error.
pub(super) async fn dispatch<E: RunningHost>(engine: &mut E, message: RequestMessage) -> Flow {
    let Envelope { client, id, body } = message;
    log::debug!("Handling the {} request {id} of {client}", body.name());

    let mut flow = Flow::Continue;
    let response = match body {
        EngineRequest::AddBlock {
            name,
            block_id,
            library,
        } => EngineResponse::AddBlock {
            result: add_block(engine, &name, block_id, library.as_deref()).map_err(Into::into),
        },

        EngineRequest::RemoveBlock { block } => EngineResponse::RemoveBlock {
            result: engine
                .graph_mut()
                .remove_block(&block)
                .await
                .map_err(Into::into),
        },

        EngineRequest::Connect { link } => EngineResponse::Connect {
            result: engine
                .graph_mut()
                .connect_blocks(&link)
                .await
                .map_err(Into::into),
        },

        EngineRequest::Disconnect { link } => EngineResponse::Disconnect {
            result: engine
                .graph_mut()
                .disconnect_link_by_id(&link)
                .await
                .map_err(Into::into),
        },

        EngineRequest::WriteInput { block, pin, value } => EngineResponse::WriteInput {
            result: engine
                .graph()
                .write_input(&block, pin, value)
                .await
                .map_err(Into::into),
        },

        EngineRequest::WriteOutput { block, pin, value } => EngineResponse::WriteOutput {
            result: engine
                .graph()
                .write_output(&block, pin, value)
                .await
                .map_err(Into::into),
        },

        EngineRequest::InspectBlock { block } => EngineResponse::InspectBlock {
            result: engine
                .graph()
                .inspect_block(&block)
                .await
                .map_err(Into::into),
        },

        EngineRequest::EvalBlock {
            name,
            inputs,
            library,
        } => EngineResponse::EvalBlock {
            result: evaluate(&name, inputs, library.as_deref())
                .await
                .map_err(Into::into),
        },

        EngineRequest::SaveProgram => EngineResponse::SaveProgram {
            result: engine.graph().save_program().await.map_err(Into::into),
        },

        EngineRequest::LoadProgram { program } => {
            log::debug!(
                "Loading a program of {} blocks and {} links",
                program.blocks.len(),
                program.links.len()
            );
            // Check the program and schedule its blocks, then wire it.
            let result = match stage_program(engine, &program) {
                Ok(staged) => engine.graph_mut().load_staged(staged).await,
                Err(err) => Err(err),
            };
            EngineResponse::LoadProgram {
                result: result.map_err(Into::into),
            }
        }

        EngineRequest::SetBlockMetadata {
            block,
            label,
            position,
        } => EngineResponse::SetBlockMetadata {
            result: engine
                .graph_mut()
                .set_metadata(&block, label, position)
                .map_err(Into::into),
        },

        EngineRequest::SetBlockEditor { block, editor } => EngineResponse::SetBlockEditor {
            result: engine
                .graph_mut()
                .set_editor(&block, editor)
                .map_err(Into::into),
        },

        EngineRequest::Watch { sender } => {
            let watch = engine.clients().next_watch();
            engine.add_watcher(watch, sender).await;
            EngineResponse::Watch { result: Ok(watch) }
        }

        EngineRequest::Unwatch { watch } => EngineResponse::Unwatch {
            result: Ok(engine.remove_watcher(watch).await),
        },

        EngineRequest::AddConnector { name } => EngineResponse::AddConnector {
            result: connectors::attach_connector(engine.connectors(), name)
                .await
                .map_err(Into::into),
        },

        EngineRequest::RemoveConnector { name } => EngineResponse::RemoveConnector {
            result: connectors::detach_connector(engine.connectors(), &name)
                .await
                .map_err(Into::into),
        },

        EngineRequest::ListConnectors => EngineResponse::ListConnectors {
            result: Ok(engine.connectors().to_vec()),
        },

        EngineRequest::Pause => {
            engine.set_execution(false);
            EngineResponse::Pause { result: Ok(()) }
        }

        EngineRequest::Resume => {
            engine.set_execution(true);
            EngineResponse::Resume { result: Ok(()) }
        }

        EngineRequest::Reset => {
            engine.graph_mut().clear();
            // Unregister first — no block can `get_connector` a
            // connector that is being stopped — then stop the handles
            // the unregistration handed back. Actor tasks are told to
            // terminate, not joined, so a block mid-`execute` may still
            // resolve-and-fault against a stopping connector; blocks
            // fault-and-retry, so this is benign.
            connectors::unregister_and_stop_connectors(engine.connectors()).await;
            EngineResponse::Reset { result: Ok(()) }
        }

        EngineRequest::Shutdown => {
            // Blocks stay scheduled, held by the closed gate, and resume
            // on the next `run()`. Connectors stay registered so the
            // re-run gets the same bindings.
            //
            // An actor mid-`execute` notices the closed gate only at its
            // next await, so it can transiently hit a connector that is
            // stopping here and fault. Benign: blocks fault-and-retry,
            // and the engine is shutting down anyway.
            engine.set_execution(false);
            connectors::stop_connectors(engine.connectors()).await;
            flow = Flow::Shutdown;
            EngineResponse::Shutdown { result: Ok(()) }
        }
    };

    if let Some(err) = response.error() {
        log::debug!(
            "The {} request {id} of {client} failed: {err}",
            response.name()
        );
    }
    engine.clients().respond(client, id, response);
    flow
}

/// Adds a block of type `name` from `lib` — or any library, if `None` —
/// under `block_id`, or a fresh id.
fn add_block<E: RunningHost>(
    engine: &mut E,
    name: &str,
    block_id: Option<Uuid>,
    lib: Option<&str>,
) -> Result<Uuid> {
    let block = get_block(name, lib)?;
    engine.schedule_desc(&block.desc, block_id)
}

/// How long an [`EngineRequest::EvalBlock`] evaluation may run, in
/// milliseconds, before it fails with [`EngineError::EvalTimeout`].
///
/// The evaluation runs on the event loop, which answers no other
/// request meanwhile, so it is bounded like a connector's start: a
/// block that waits on an input the evaluation did not supply holds the
/// engine up this long, not forever.
pub(super) const EVAL_TIMEOUT_MILLIS: u64 = 5000;

/// Evaluates block type `name` from `lib` once, with `inputs`.
///
/// The evaluation runs block code on the event loop itself, not in an
/// actor, so it gets the actors' guard: a panic is caught and fails the
/// request with [`EngineError::EvalPanicked`] (on native targets; a
/// panic aborts a `wasm32` instance), and an evaluation still running
/// after [`EVAL_TIMEOUT_MILLIS`] is dropped and fails it with
/// [`EngineError::EvalTimeout`].
async fn evaluate(
    name: &str,
    inputs: Vec<libhaystack::val::Value>,
    lib: Option<&str>,
) -> Result<Vec<libhaystack::val::Value>> {
    let block = get_block(name, lib)?;
    let desc = &block.desc;
    tokio::select! {
        outcome = catch_panic(eval_block(desc, inputs)) => {
            outcome.map_err(|message| EngineError::EvalPanicked {
                library: desc.library.clone(),
                name: desc.name.clone(),
                message,
            })?
        }
        () = sleep_millis(EVAL_TIMEOUT_MILLIS) => Err(EngineError::EvalTimeout {
            library: desc.library.clone(),
            name: desc.name.clone(),
            millis: EVAL_TIMEOUT_MILLIS,
        }
        .into()),
    }
}
