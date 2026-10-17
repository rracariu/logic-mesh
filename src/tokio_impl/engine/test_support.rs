// Copyright (c) 2022-2026, Radu Racariu.

//! Helpers shared by the single- and multi-threaded engine tests.

use std::future::Future;
use std::panic::{AssertUnwindSafe, resume_unwind};
use std::thread;
use std::time::Duration;

use futures::FutureExt;
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
    time::timeout,
};
use uuid::Uuid;

use crate::base::engine::{Engine, messages::EngineMessage};
use crate::base::program::{Program, ProgramBlock};
use crate::single_threaded::Messages;

/// How long a test waits for a reply before declaring the engine stuck.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// A test's end of an engine message channel.
pub(crate) struct Client {
    /// The channel id the engine addresses replies to.
    pub(crate) id: Uuid,
    sender: Sender<Messages>,
    receiver: Receiver<Messages>,
}

impl Client {
    /// Sends `request` and returns its reply, failing the test if the
    /// engine does not answer within [`REPLY_TIMEOUT`].
    pub(crate) async fn request(&mut self, request: Messages) -> Messages {
        self.send(request).await;
        timeout(REPLY_TIMEOUT, self.receiver.recv())
            .await
            .expect("the engine answers the request")
            .expect("the reply channel is open")
    }

    /// Sends a message that has no reply.
    pub(crate) async fn send(&self, message: Messages) {
        self.sender
            .send(message)
            .await
            .expect("the engine is receiving");
    }
}

/// Opens a message channel on `engine`.
pub(crate) fn connect<E: Engine<Channel = Sender<Messages>>>(engine: &mut E) -> Client {
    let (sender, receiver) = mpsc::channel(32);
    let id = Uuid::new_v4();
    let sender = engine.create_message_channel(id, sender);
    Client {
        id,
        sender,
        receiver,
    }
}

/// Runs `engine` while `driver` talks to it from a runtime on its own
/// thread, then shuts the engine down and hands it back, idle.
///
/// The [`Shutdown`](EngineMessage::Shutdown) is sent here, after the
/// driver finishes — drivers never send it themselves, or it would stay
/// queued and end the next `run()`. A panicking driver still shuts the
/// engine down, and the panic is re-raised once `run()` returns, so a
/// failed assertion fails the test instead of hanging it.
pub(crate) async fn drive<E, F, Fut>(mut engine: E, driver: F) -> E
where
    E: Engine<Channel = Sender<Messages>>,
    F: FnOnce(Client) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let client = connect(&mut engine);
    let shutdown = client.sender.clone();
    let thread = thread::spawn(move || {
        let runtime = Runtime::new().expect("driver runtime");
        runtime.block_on(async move {
            let outcome = AssertUnwindSafe(driver(client)).catch_unwind().await;
            let _ = shutdown.send(EngineMessage::Shutdown).await;
            outcome
        })
    });

    let engine = engine.run().await;
    if let Err(panic) = thread.join().expect("driver thread joins") {
        resume_unwind(panic);
    }
    engine
}

/// A program of unlinked core `Add` blocks keyed by `ids`, verbatim.
pub(crate) fn add_program<'a>(ids: impl IntoIterator<Item = &'a str>) -> Program {
    Program {
        name: None,
        description: None,
        blocks: ids
            .into_iter()
            .map(|id| {
                (
                    id.to_string(),
                    ProgramBlock {
                        name: "Add".to_string(),
                        lib: "core".to_string(),
                        label: None,
                        positions: None,
                        inputs: Default::default(),
                        outputs: Default::default(),
                    },
                )
            })
            .collect(),
        links: Default::default(),
    }
}
