// Copyright (c) 2022-2026, Radu Racariu.

//! Helpers shared by the single- and multi-threaded engine tests.

use std::future::Future;
use std::panic::{AssertUnwindSafe, resume_unwind};
use std::thread;
use std::time::Duration;

use futures::FutureExt;
use libhaystack::val::Value;
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
    time::{sleep, timeout},
};
use uuid::Uuid;

use crate::base::block::{BlockProps, connect::connect_output};
use crate::base::engine::{
    Engine,
    messages::{BlockDefinition, EngineMessage},
};
use crate::base::program::{Program, ProgramBlock};
use crate::blocks::math::Add;
use crate::single_threaded::Messages;
use crate::tokio_impl::{ReaderImpl, WriterImpl};

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

    /// Inspects `block`, failing the test if the engine reports an error.
    pub(crate) async fn inspect(&mut self, block: Uuid) -> BlockDefinition {
        match self
            .request(EngineMessage::InspectBlockReq(self.id, block))
            .await
        {
            EngineMessage::InspectBlockRes(Ok(definition)) => definition,
            other => panic!("Expected InspectBlockRes(Ok), got {:?}", other),
        }
    }

    /// The current value of `block`'s output `pin`.
    pub(crate) async fn output(&mut self, block: Uuid, pin: &str) -> Value {
        self.inspect(block).await.outputs[pin].val.clone()
    }

    /// Writes `value` to `block`'s output `pin`.
    pub(crate) async fn write_output(&mut self, block: Uuid, pin: &str, value: Value) {
        let request = EngineMessage::WriteBlockOutputReq(self.id, block, pin.to_string(), value);
        match self.request(request).await {
            EngineMessage::WriteBlockOutputRes(Ok(_)) => {}
            other => panic!("Expected WriteBlockOutputRes(Ok), got {:?}", other),
        }
    }

    /// Polls `block` until its output `pin` holds `expected`, failing the
    /// test after a few seconds.
    pub(crate) async fn wait_for_output(&mut self, block: Uuid, pin: &str, expected: Value) {
        for _ in 0..100 {
            if self.output(block, pin).await == expected {
                return;
            }
            sleep(Duration::from_millis(20)).await;
        }
        panic!("{block}.{pin} never became {expected:?}");
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

/// Schedules two `Add` blocks on `engine`, the first one's `out` linked
/// to the second one's `in0`, and returns their ids. Writing the first
/// block's `out` makes the second one's `out` follow — but only once the
/// second block executes.
pub(crate) fn schedule_linked_adds<E>(engine: &mut E) -> (Uuid, Uuid)
where
    E: Engine<Writer = WriterImpl, Reader = ReaderImpl>,
{
    let mut source = Add::new();
    let mut target = Add::new();
    connect_output(&mut source.out, target.inputs_mut()[0]).expect("linked");
    let ids = (*source.id(), *target.id());
    engine.schedule(source).expect("source scheduled");
    engine.schedule(target).expect("target scheduled");
    ids
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

/// The [lifecycle contract](crate::base::engine#lifecycle-messages),
/// written once against the [`Engine`] trait and run by each engine's
/// tests.
pub(crate) mod lifecycle {
    use assert_matches::assert_matches;

    use super::*;
    use crate::base::engine::messages::EngineMessage::{
        AddBlockReq, AddBlockRes, InspectBlockReq, InspectBlockRes, Pause, Reset, Resume,
    };

    /// An engine the lifecycle tests can drive.
    pub(crate) trait TestEngine:
        Engine<Channel = Sender<Messages>, Writer = WriterImpl, Reader = ReaderImpl>
    {
    }
    impl<E> TestEngine for E where
        E: Engine<Channel = Sender<Messages>, Writer = WriterImpl, Reader = ReaderImpl>
    {
    }

    /// Blocks survive a shutdown, and values flow again on the next run.
    pub(crate) async fn blocks_survive_shutdown<E: TestEngine>(mut engine: E) {
        let (source, target) = schedule_linked_adds(&mut engine);

        let engine = drive(engine, move |mut client| async move {
            client.write_output(source, "out", 1.into()).await;
            client.wait_for_output(target, "out", 1.into()).await;
        })
        .await;

        drive(engine, move |mut client| async move {
            assert_eq!(
                client.output(target, "out").await,
                1.into(),
                "the target kept its value across the shutdown"
            );
            client.write_output(source, "out", 2.into()).await;
            client.wait_for_output(target, "out", 2.into()).await;
        })
        .await;
    }

    /// While paused no block executes, yet every request is answered;
    /// resuming lets the held value through.
    pub(crate) async fn pause_holds_blocks_and_answers_requests<E: TestEngine>(mut engine: E) {
        let (source, target) = schedule_linked_adds(&mut engine);

        drive(engine, move |mut client| async move {
            client.write_output(source, "out", 1.into()).await;
            client.wait_for_output(target, "out", 1.into()).await;

            client.send(Pause).await;
            // Answered while paused, and lands on the source directly.
            client.write_output(source, "out", 2.into()).await;
            assert_eq!(client.output(source, "out").await, 2.into());
            // The target does not execute, so it never picks the value
            // up — however many requests drive the engine meanwhile.
            for _ in 0..10 {
                sleep(Duration::from_millis(20)).await;
                assert_eq!(
                    client.output(target, "out").await,
                    1.into(),
                    "no block executes while paused"
                );
            }

            client.send(Resume).await;
            client.wait_for_output(target, "out", 2.into()).await;
        })
        .await;
    }

    /// A reset while paused removes every block, the engine keeps
    /// answering, and blocks added afterwards run once resumed.
    pub(crate) async fn reset_while_paused<E: TestEngine>(mut engine: E) {
        let (source, _) = schedule_linked_adds(&mut engine);

        drive(engine, move |mut client| async move {
            client.send(Pause).await;
            client.send(Reset).await;

            let reply = client.request(InspectBlockReq(client.id, source)).await;
            assert_matches!(reply, InspectBlockRes(Err(_)), "reset removed the block");

            let reply = client
                .request(AddBlockReq(client.id, "Add".to_string(), None, None))
                .await;
            let added = match reply {
                AddBlockRes(Ok(id)) => id,
                other => panic!("Expected AddBlockRes(Ok), got {:?}", other),
            };

            client.send(Resume).await;
            client.write_output(added, "out", 3.into()).await;
            assert_eq!(client.output(added, "out").await, 3.into());
        })
        .await;
    }
}
