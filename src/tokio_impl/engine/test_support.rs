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

use crate::base::block::{BlockProps, connect_output};
use crate::base::engine::{
    Engine,
    messages::{BlockDefinition, EngineMessage},
};
use crate::base::program::{LinkData, Program, ProgramBlock};
use crate::blocks::math::Add;
use crate::tokio_impl::engine::Messages;
use crate::tokio_impl::{ReaderImpl, WriterImpl};

/// How long a test waits for a reply before declaring the engine stuck.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// How often, and how many times, the `wait_for_*` helpers poll: long
/// enough to outlast a panic retry delay.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const POLLS: usize = 250;

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

    /// Links `source`'s `source_pin` to `target`'s `target_pin`, under
    /// `id` if given, returning the engine's answer.
    pub(crate) async fn connect(
        &mut self,
        id: Option<&str>,
        (source, source_pin): (Uuid, &str),
        (target, target_pin): (Uuid, &str),
    ) -> Result<LinkData, String> {
        let link = LinkData {
            id: id.map(str::to_string),
            source_block_uuid: source.to_string(),
            target_block_uuid: target.to_string(),
            source_block_pin_name: source_pin.to_string(),
            target_block_pin_name: target_pin.to_string(),
        };
        match self
            .request(EngineMessage::ConnectBlocksReq(self.id, link))
            .await
        {
            EngineMessage::ConnectBlocksRes(res) => res,
            other => panic!("Expected ConnectBlocksRes, got {:?}", other),
        }
    }

    /// Removes link `id`, returning the engine's answer.
    pub(crate) async fn remove_link(&mut self, id: Uuid) -> Result<bool, String> {
        match self
            .request(EngineMessage::RemoveLinkReq(self.id, id))
            .await
        {
            EngineMessage::RemoveLinkRes(res) => res,
            other => panic!("Expected RemoveLinkRes, got {:?}", other),
        }
    }

    /// Removes block `id`, returning the engine's answer.
    pub(crate) async fn remove_block(&mut self, id: Uuid) -> Result<Uuid, String> {
        match self
            .request(EngineMessage::RemoveBlockReq(self.id, id))
            .await
        {
            EngineMessage::RemoveBlockRes(res) => res,
            other => panic!("Expected RemoveBlockRes, got {:?}", other),
        }
    }

    /// Loads `program`, returning the engine's answer.
    pub(crate) async fn load(&mut self, program: Program) -> Result<(), String> {
        match self
            .request(EngineMessage::LoadProgramReq(self.id, program))
            .await
        {
            EngineMessage::LoadProgramRes(res) => res,
            other => panic!("Expected LoadProgramRes, got {:?}", other),
        }
    }

    /// Saves the running program, failing the test if the engine
    /// reports an error.
    pub(crate) async fn save(&mut self) -> Program {
        match self
            .request(EngineMessage::GetCurrentProgramReq(self.id))
            .await
        {
            EngineMessage::GetCurrentProgramRes(Ok(program)) => program,
            other => panic!("Expected GetCurrentProgramRes(Ok), got {:?}", other),
        }
    }

    /// Writes `value` to `block`'s input `pin`.
    pub(crate) async fn write_input(&mut self, block: Uuid, pin: &str, value: Value) {
        let request = EngineMessage::WriteBlockInputReq(self.id, block, pin.to_string(), value);
        match self.request(request).await {
            EngineMessage::WriteBlockInputRes(Ok(_)) => {}
            other => panic!("Expected WriteBlockInputRes(Ok), got {:?}", other),
        }
    }

    /// Polls `block` until its output `pin` holds `expected`, failing the
    /// test after a few seconds.
    pub(crate) async fn wait_for_output(&mut self, block: Uuid, pin: &str, expected: Value) {
        for _ in 0..POLLS {
            if self.output(block, pin).await == expected {
                return;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!("{block}.{pin} never became {expected:?}");
    }

    /// Polls `block` until its state label reads `state`, failing the
    /// test after a few seconds, and returns that inspection.
    pub(crate) async fn wait_for_state(&mut self, block: Uuid, state: &str) -> BlockDefinition {
        for _ in 0..POLLS {
            let definition = self.inspect(block).await;
            if definition.state == state {
                return definition;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!("{block} never reached state {state}");
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
    let mut program = Program::new();
    program.blocks = ids
        .into_iter()
        .map(|id| (id.to_string(), ProgramBlock::new("Add", "core")))
        .collect();
    program
}

/// Blocks for the actor-level fault tests of both engines.
pub(crate) mod faults {
    use crate::base::block::{Block, BlockProps, BlockState};
    use crate::blocks::OutputImpl;

    /// The fault reason a [`FaultOnce`] block leaves.
    pub(crate) const REASON: &str = "faulted once";

    /// Faults itself on its first execute cycle, then waits forever: a
    /// block left in an ordinary fault, not a panic hold, whose next
    /// cycle never completes.
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct FaultOnce {
        #[output(kind = "Number")]
        pub(crate) out: OutputImpl,
        faulted: bool,
    }

    impl Block for FaultOnce {
        async fn execute(&mut self) {
            if self.faulted {
                std::future::pending::<()>().await;
            }
            self.faulted = true;
            self.set_state(BlockState::fault(REASON));
        }
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

/// Engine pin writes, written once against the [`Engine`] trait and run
/// by each engine's tests: a write to any input runs the block, linked
/// or not.
pub(crate) mod pin_writes {
    use super::lifecycle::TestEngine;
    use super::*;

    /// A number carrying `unit`.
    fn with_unit(value: f64, unit: &str) -> Value {
        Value::make_number_unit(value, unit.into())
    }

    /// A block with no links at all recomputes on a write to one of its
    /// inputs.
    pub(crate) async fn writing_an_unlinked_block_recomputes_it<E: TestEngine>(mut engine: E) {
        let add = Add::new();
        let id = *add.id();
        engine.schedule(add).expect("scheduled");

        drive(engine, move |mut client| async move {
            client.write_input(id, "in0", 5.into()).await;
            client.wait_for_output(id, "out", 5.into()).await;
            client.write_input(id, "in1", 2.into()).await;
            client.wait_for_output(id, "out", 7.into()).await;
        })
        .await;
    }

    /// A write to a constant input of a linked block recomputes it at
    /// once, without waiting for a linked input to change.
    pub(crate) async fn writing_a_constant_input_recomputes_a_linked_block<E: TestEngine>(
        mut engine: E,
    ) {
        let (source, target) = schedule_linked_adds(&mut engine);

        drive(engine, move |mut client| async move {
            client.write_output(source, "out", 1.into()).await;
            client.wait_for_output(target, "out", 1.into()).await;

            client.write_input(target, "in1", 2.into()).await;
            client.wait_for_output(target, "out", 3.into()).await;
        })
        .await;
    }

    /// Faults only clear on a completed cycle, so a fix written to an
    /// unlinked input clears the fault only because the write runs the
    /// block.
    pub(crate) async fn a_fix_written_to_an_unlinked_input_clears_a_fault<E: TestEngine>(
        mut engine: E,
    ) {
        let add = Add::new();
        let id = *add.id();
        engine.schedule(add).expect("scheduled");

        drive(engine, move |mut client| async move {
            client.write_input(id, "in0", with_unit(1.0, "m")).await;
            client.write_input(id, "in1", with_unit(1.0, "s")).await;
            client.wait_for_state(id, "fault").await;

            client.write_input(id, "in1", with_unit(2.0, "m")).await;
            client.wait_for_state(id, "running").await;
            client.wait_for_output(id, "out", with_unit(3.0, "m")).await;
        })
        .await;
    }
}

/// `program` as JSON, for comparing two programs whole.
pub(crate) fn json(program: &Program) -> serde_json::Value {
    serde_json::to_value(program).expect("a program serializes")
}

/// A link from `source`'s `out` to `target`'s `pin`, naming `id` if
/// given, for a [`Program`].
pub(crate) fn program_link(source: Uuid, target: Uuid, pin: &str, id: Option<Uuid>) -> LinkData {
    LinkData {
        id: id.map(|id| id.to_string()),
        source_block_uuid: source.to_string(),
        target_block_uuid: target.to_string(),
        source_block_pin_name: "out".to_string(),
        target_block_pin_name: pin.to_string(),
    }
}

/// Link ids, written once against the [`Engine`] trait and run by each
/// engine's tests: a link keeps the id its creator names — over a
/// connect request or in a loaded program — and ids stay unique.
pub(crate) mod link_ids {
    use assert_matches::assert_matches;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::error::EngineError;

    /// Two unlinked `Add` blocks, scheduled.
    fn schedule_adds<E: TestEngine>(engine: &mut E) -> (Uuid, Uuid) {
        let (source, target) = (Add::new(), Add::new());
        let ids = (*source.id(), *target.id());
        engine.schedule(source).expect("source scheduled");
        engine.schedule(target).expect("target scheduled");
        ids
    }

    /// A connect request naming an id creates the link under that id,
    /// and the link can be removed by it.
    pub(crate) async fn connecting_keeps_a_named_link_id<E: TestEngine>(mut engine: E) {
        let (source, target) = schedule_adds(&mut engine);
        let id = Uuid::new_v4();

        drive(engine, move |mut client| async move {
            let named = id.to_string();
            let link = client
                .connect(Some(&named), (source, "out"), (target, "in0"))
                .await
                .expect("linked");
            assert_eq!(link.id.as_deref(), Some(named.as_str()));
            let saved = client.save().await;
            assert_eq!(saved.links.keys().collect::<Vec<_>>(), vec![&named]);

            assert_eq!(client.remove_link(id).await, Ok(true));
            assert!(!client.inspect(target).await.inputs["in0"].is_connected);
            assert!(client.save().await.links.is_empty());
        })
        .await;
    }

    /// An id another link has — wired over a request, or before the
    /// blocks were scheduled — is rejected, and so is one that does not
    /// parse.
    pub(crate) async fn connecting_rejects_a_taken_or_invalid_link_id<E: TestEngine>(
        mut engine: E,
    ) {
        let (linked_source, linked_target) = schedule_linked_adds(&mut engine);
        let (source, target) = schedule_adds(&mut engine);

        drive(engine, move |mut client| async move {
            let prewired = client.save().await.links.into_keys().next();
            let prewired = prewired.expect("the pre-wired link is saved");
            let reply = client
                .connect(Some(&prewired), (source, "out"), (target, "in0"))
                .await;
            let taken = Uuid::parse_str(&prewired).expect("a uuid");
            assert_matches!(
                reply,
                Err(msg) if msg == EngineError::DuplicateLinkId { id: taken }.to_string()
            );

            let id = Uuid::new_v4().to_string();
            client
                .connect(Some(&id), (source, "out"), (target, "in0"))
                .await
                .expect("linked");
            let reply = client
                .connect(Some(&id), (linked_source, "out"), (linked_target, "in1"))
                .await;
            assert_matches!(reply, Err(msg) if msg.contains("already exists"));

            let reply = client
                .connect(Some("link-1"), (source, "out"), (target, "in1"))
                .await;
            assert_matches!(reply, Err(msg) if msg.starts_with("Invalid link uuid 'link-1'"));

            // None of the rejected links was wired.
            assert_eq!(client.save().await.links.len(), 2);
        })
        .await;
    }

    /// A loaded link keeps the id it was saved under — the key the
    /// program's links are keyed by, when the link names no id itself —
    /// and can be removed by it.
    pub(crate) async fn a_loaded_link_keeps_its_saved_id<E: TestEngine>(engine: E) {
        let (source, target) = (Uuid::new_v4(), Uuid::new_v4());
        let (keyed, named) = (Uuid::new_v4(), Uuid::new_v4());
        let mut program = add_program([source.to_string().as_str(), &target.to_string()]);
        program
            .links
            .insert(keyed.to_string(), program_link(source, target, "in0", None));
        program.links.insert(
            named.to_string(),
            program_link(source, target, "in1", Some(named)),
        );

        drive(engine, move |mut client| async move {
            client.load(program).await.expect("loaded");
            let saved = client.save().await;
            let mut ids: Vec<_> = saved.links.keys().cloned().collect();
            ids.sort();
            let mut expected = vec![keyed.to_string(), named.to_string()];
            expected.sort();
            assert_eq!(ids, expected);

            assert_eq!(client.remove_link(keyed).await, Ok(true));
            assert!(!client.inspect(target).await.inputs["in0"].is_connected);
            assert_eq!(client.remove_link(named).await, Ok(true));
            assert!(!client.inspect(target).await.inputs["in1"].is_connected);
        })
        .await;
    }

    /// A program naming one link id twice, or an id a link on the
    /// engine already has, is rejected.
    pub(crate) async fn a_program_reusing_a_link_id_is_rejected<E: TestEngine>(mut engine: E) {
        let (scheduled_source, scheduled_target) = schedule_adds(&mut engine);
        let taken = Uuid::new_v4();
        // Two linked blocks, the links named `taken`.
        let program_with = |pins: &[&str]| {
            let (source, target) = (Uuid::new_v4(), Uuid::new_v4());
            let mut program = add_program([source.to_string().as_str(), &target.to_string()]);
            for pin in pins {
                let link = program_link(source, target, pin, Some(taken));
                program.links.insert(Uuid::new_v4().to_string(), link);
            }
            program
        };
        let twice = program_with(&["in0", "in1"]);
        let reused = program_with(&["in0"]);

        drive(engine, move |mut client| async move {
            let reply = client.load(twice).await;
            assert_eq!(
                reply,
                Err(EngineError::DuplicateLinkId { id: taken }.to_string())
            );

            client
                .connect(
                    Some(&taken.to_string()),
                    (scheduled_source, "out"),
                    (scheduled_target, "in0"),
                )
                .await
                .expect("linked");
            let reply = client.load(reused).await;
            assert_eq!(
                reply,
                Err(EngineError::DuplicateLinkId { id: taken }.to_string())
            );
        })
        .await;
    }
}

/// Program save and load, written once against the [`Engine`] trait and
/// run by each engine's tests: a saved program loads back as it was
/// saved, and saves the same again.
pub(crate) mod round_trip {
    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::BlockDesc;
    use crate::base::engine::messages::EngineMessage::Reset;
    use crate::base::program::data::PROGRAM_1_0_JSON;
    use crate::base::program::{EditorMetadata, PinValue, Position};

    /// The ids in a [`sample`] program.
    #[derive(Clone, Copy)]
    struct Ids {
        /// An `Add` with two constant inputs.
        first: Uuid,
        /// An `Add` both of whose first two inputs follow `first`.
        second: Uuid,
        /// The link into `second.in0`, saved under its key alone.
        keyed: Uuid,
        /// The link into `second.in1`, naming its id.
        named: Uuid,
    }

    /// Editor metadata exercising every JSON kind.
    fn editor_metadata(kind: &str) -> EditorMetadata {
        let editor = serde_json::json!({
            "widget": {
                "kind": kind,
                "config": {"series": [{"label": "a", "address": null}], "min": -1, "step": 0.1},
                "visible": true,
            },
        });
        match editor {
            serde_json::Value::Object(editor) => editor,
            _ => unreachable!("an object literal"),
        }
    }

    /// A program using everything a save keeps: a name and description,
    /// block versions, labels, positions and editor metadata, constant
    /// inputs, and links.
    fn sample() -> (Program, Ids) {
        let ids = Ids {
            first: Uuid::new_v4(),
            second: Uuid::new_v4(),
            keyed: Uuid::new_v4(),
            named: Uuid::new_v4(),
        };
        let mut program = add_program([ids.first.to_string().as_str(), &ids.second.to_string()]);
        program.name = Some("round trip".to_string());
        program.description = Some("Saved, loaded, and saved again.".to_string());
        for (id, label, x) in [(ids.first, "First", 10.0), (ids.second, "Second", 200.0)] {
            let block = program
                .blocks
                .get_mut(&id.to_string())
                .expect("in the program");
            block.ver = Some(BlockDesc::DEFAULT_VER.to_string());
            block.label = Some(label.to_string());
            block.position = Some(Position { x, y: 20.5 });
            block.editor = Some(editor_metadata(label));
        }
        let first = program
            .blocks
            .get_mut(&ids.first.to_string())
            .expect("in the program");
        for (pin, value) in [("in0", 42), ("in1", 3)] {
            let value = PinValue {
                value: value.into(),
                is_connected: false,
            };
            first.inputs.insert(pin.to_string(), value);
        }
        program.links.insert(
            ids.keyed.to_string(),
            program_link(ids.first, ids.second, "in0", None),
        );
        program.links.insert(
            ids.named.to_string(),
            program_link(ids.first, ids.second, "in1", Some(ids.named)),
        );
        (program, ids)
    }

    /// Polls the running program until it saves as `expected`, failing
    /// the test after a few seconds.
    pub(crate) async fn wait_for_save(client: &mut Client, expected: &Program) {
        let expected = json(expected);
        let mut saved = serde_json::Value::Null;
        for _ in 0..POLLS {
            saved = json(&client.save().await);
            if saved == expected {
                return;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!("the program never saved as {expected:#}, last saved as {saved:#}");
    }

    /// Checks that `saved` keeps everything [`sample`] put in `program`.
    fn assert_keeps_the_sample(saved: &Program, program: &Program, ids: Ids) {
        assert_eq!(saved.version, Program::FORMAT_VERSION);
        assert_eq!(saved.name, program.name);
        assert_eq!(saved.description, program.description);
        assert_eq!(
            saved.blocks.keys().collect::<Vec<_>>(),
            program.blocks.keys().collect::<Vec<_>>()
        );
        for (id, block) in &program.blocks {
            let saved = &saved.blocks[id];
            assert_eq!((&saved.name, &saved.lib), (&block.name, &block.lib));
            assert_eq!(saved.ver, block.ver);
            assert_eq!(saved.label, block.label);
            assert_eq!(saved.position, block.position);
            assert_eq!(saved.editor, block.editor);
            for (pin, value) in &block.inputs {
                assert_eq!(saved.inputs[pin].value, value.value, "{id}.{pin}");
            }
        }
        assert_eq!(
            saved.links.keys().collect::<Vec<_>>(),
            program.links.keys().collect::<Vec<_>>()
        );
        for (id, link) in &program.links {
            let saved = &saved.links[id];
            assert_eq!(saved.id.as_deref(), Some(id.as_str()));
            assert_eq!(saved.source_block_uuid, link.source_block_uuid);
            assert_eq!(saved.target_block_uuid, link.target_block_uuid);
            assert_eq!(saved.source_block_pin_name, link.source_block_pin_name);
            assert_eq!(saved.target_block_pin_name, link.target_block_pin_name);
        }
        assert!(saved.blocks[&ids.second.to_string()].inputs["in0"].is_connected);
    }

    /// A loaded program saves with everything it was loaded with; that
    /// save loads back and saves the same again; and a saved link id
    /// removes the link.
    pub(crate) async fn a_saved_program_reloads_unchanged<E: TestEngine>(engine: E) {
        let (program, ids) = sample();

        drive(engine, move |mut client| async move {
            client.load(program.clone()).await.expect("loaded");
            client.wait_for_output(ids.second, "out", 90.into()).await;
            let saved = client.save().await;
            assert_keeps_the_sample(&saved, &program, ids);

            client.send(Reset).await;
            client.load(saved.clone()).await.expect("reloaded");
            wait_for_save(&mut client, &saved).await;

            assert_eq!(client.remove_link(ids.keyed).await, Ok(true));
            assert!(!client.inspect(ids.second).await.inputs["in0"].is_connected);
        })
        .await;
    }

    /// A program saved by logic-mesh 1.0 — no format version, the
    /// position keyed `positions`, no block versions — loads, and saves
    /// in the current format with everything it was loaded with.
    pub(crate) async fn a_1_0_program_loads_and_saves_in_the_current_format<E: TestEngine>(
        engine: E,
    ) {
        let program: Program = serde_json::from_str(PROGRAM_1_0_JSON).expect("a 1.0 save parses");
        let first = Uuid::parse_str("00000000-0000-4000-8000-000000000001").expect("uuid");
        let second = Uuid::parse_str("00000000-0000-4000-8000-000000000002").expect("uuid");
        let link = "00000000-0000-4000-8000-0000000000aa";

        drive(engine, move |mut client| async move {
            client.load(program).await.expect("a 1.0 program loads");
            client.wait_for_output(second, "out", 20.into()).await;

            let saved = client.save().await;
            assert_eq!(saved.version, Program::FORMAT_VERSION);
            assert_eq!(saved.name.as_deref(), Some("Demand sum"));
            let block = &saved.blocks[&first.to_string()];
            assert_eq!(block.ver.as_deref(), Some(BlockDesc::DEFAULT_VER));
            assert_eq!(block.label.as_deref(), Some("Zone A + B"));
            assert_eq!(block.position, Some(Position::new(81.0, -72.5)));
            assert_eq!(block.inputs["in0"].value, 12.into());
            assert_eq!(
                saved.blocks[&second.to_string()].position,
                Some(Position::new(267.0, 25.0))
            );
            assert_eq!(saved.links[link].id.as_deref(), Some(link));
            let written = json(&saved);
            assert!(written["blocks"][first.to_string()]["positions"].is_null());
            assert_eq!(written["version"], Program::FORMAT_VERSION);
        })
        .await;
    }

    /// A block saved at another version of its type that is compatible
    /// with the registered one loads, and saves at the registered one.
    pub(crate) async fn a_block_saved_at_a_compatible_version_loads<E: TestEngine>(engine: E) {
        let id = Uuid::new_v4();
        let mut program = add_program([id.to_string().as_str()]);
        for block in program.blocks.values_mut() {
            block.ver = Some("1.7.3".to_string());
        }

        drive(engine, move |mut client| async move {
            client
                .load(program)
                .await
                .expect("a compatible version loads");
            let saved = client.save().await;
            assert_eq!(
                saved.blocks[&id.to_string()].ver.as_deref(),
                Some(BlockDesc::DEFAULT_VER)
            );
        })
        .await;
    }

    /// A reset forgets the loaded program's name and description.
    pub(crate) async fn reset_forgets_the_program_identity<E: TestEngine>(engine: E) {
        let (program, _) = sample();

        drive(engine, move |mut client| async move {
            client.load(program).await.expect("loaded");
            assert!(client.save().await.name.is_some());

            client.send(Reset).await;
            let saved = client.save().await;
            assert_eq!((saved.name, saved.description), (None, None));
            assert!(saved.blocks.is_empty() && saved.links.is_empty());
        })
        .await;
    }

    /// A program scheduled while idle keeps its name, description and
    /// link ids too.
    pub(crate) async fn a_program_scheduled_while_idle_saves_its_identity<E: TestEngine>(
        mut engine: E,
    ) {
        let (program, ids) = sample();
        engine.schedule_program_blocks(&program).expect("scheduled");

        drive(engine, move |mut client| async move {
            let saved = client.save().await;
            assert_eq!(saved.name, program.name);
            assert_eq!(saved.description, program.description);
            assert_eq!(
                saved.links.keys().collect::<Vec<_>>(),
                program.links.keys().collect::<Vec<_>>()
            );
            assert_eq!(client.remove_link(ids.named).await, Ok(true));
        })
        .await;
    }
}

/// Program loads are all or nothing, written once against the
/// [`Engine`] trait and run by each engine's tests: a program that
/// fails to load leaves the engine as it was.
pub(crate) mod atomic_loads {
    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::BlockDesc;
    use crate::base::error::{ConnectError, EngineError, LinkEnd, RegistryError};
    use crate::base::program::PinValue;

    /// Two fresh ids, in the order a program's maps hold them: the
    /// first is staged before the second.
    fn ordered_ids() -> (Uuid, Uuid) {
        let (a, b) = (Uuid::new_v4(), Uuid::new_v4());
        if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// A program of two `Add` blocks, in staging order, and their ids.
    fn two_adds() -> (Program, Uuid, Uuid) {
        let (first, second) = ordered_ids();
        let program = add_program([first.to_string().as_str(), &second.to_string()]);
        (program, first, second)
    }

    /// Programs failing each check a load makes before scheduling a
    /// block, with the error each fails with. `outside` is a block
    /// already on the engine.
    fn failing_programs(outside: Uuid) -> Vec<(&'static str, Program, String)> {
        let mut cases = Vec::new();

        let (mut program, _, _) = two_adds();
        program.version = Program::FORMAT_VERSION + 1;
        let err = EngineError::UnsupportedProgramVersion {
            found: Program::FORMAT_VERSION + 1,
            supported: Program::FORMAT_VERSION,
        };
        cases.push(("a newer format version", program, err.to_string()));

        for saved in ["2.0.0", "0.9.0"] {
            let (mut program, _, second) = two_adds();
            let block = program.blocks.get_mut(&second.to_string()).expect("block");
            block.ver = Some(saved.to_string());
            let err = EngineError::BlockVersionMismatch {
                block: second,
                name: "Add".to_string(),
                saved: saved.to_string(),
                registered: BlockDesc::DEFAULT_VER.to_string(),
            };
            cases.push(("an incompatible block version", program, err.to_string()));
        }

        let (mut program, _, second) = two_adds();
        let block = program.blocks.get_mut(&second.to_string()).expect("block");
        block.name = "NoSuchBlock".to_string();
        let err = RegistryError::BlockNotFound {
            library: "core".to_string(),
            name: "NoSuchBlock".to_string(),
        };
        cases.push(("an unknown block type", program, err.to_string()));

        let (mut program, first, second) = two_adds();
        let id = Uuid::new_v4();
        let link = program_link(first, second, "nope", Some(id));
        program.links.insert(id.to_string(), link);
        let err = EngineError::PinNotFound {
            end: LinkEnd::Target,
            block: second,
            pin: "nope".to_string(),
        };
        cases.push(("a link to an unknown pin", program, err.to_string()));

        let (mut program, first, _) = two_adds();
        let id = Uuid::new_v4();
        let link = program_link(first, outside, "in0", Some(id));
        program.links.insert(id.to_string(), link);
        let err = EngineError::LinkBlockNotInProgram {
            link: id,
            end: LinkEnd::Target,
            block: outside,
        };
        cases.push(("a link out of the program", program, err.to_string()));

        for output in [false, true] {
            let (mut program, _, second) = two_adds();
            let block = program.blocks.get_mut(&second.to_string()).expect("block");
            let pins = if output {
                &mut block.outputs
            } else {
                &mut block.inputs
            };
            let value = PinValue {
                value: 1.into(),
                is_connected: false,
            };
            pins.insert("nope".to_string(), value);
            let (block, pin) = (second, "nope".to_string());
            let err = if output {
                EngineError::OutputNotFound { block, pin }
            } else {
                EngineError::InputNotFound { block, pin }
            };
            cases.push(("a value for an unknown pin", program, err.to_string()));
        }

        let (mut program, first, second) = two_adds();
        let (existing, repeated) = ordered_ids();
        for id in [existing, repeated] {
            let link = program_link(first, second, "in0", Some(id));
            program.links.insert(id.to_string(), link);
        }
        let err = EngineError::DuplicateLink {
            link: repeated,
            existing,
        };
        cases.push(("two links joining the same pins", program, err.to_string()));

        cases
    }

    /// A program failing any check loads nothing: the engine saves as it
    /// did before, and goes on loading.
    pub(crate) async fn a_program_failing_its_checks_loads_nothing<E: TestEngine>(mut engine: E) {
        let (_, outside) = schedule_linked_adds(&mut engine);

        drive(engine, move |mut client| async move {
            let before = json(&client.save().await);
            for (case, program, expected) in failing_programs(outside) {
                let reply = client.load(program).await;
                assert_eq!(reply, Err(expected), "{case}");
                assert_eq!(json(&client.save().await), before, "{case} loads nothing");
            }

            let (program, first, _) = two_adds();
            client.load(program).await.expect("a valid program loads");
            client.inspect(first).await;
        })
        .await;
    }

    /// A program failing any check while idle schedules nothing.
    pub(crate) async fn a_program_failing_its_checks_schedules_nothing<E: TestEngine>(
        mut engine: E,
    ) {
        let (_, outside) = schedule_linked_adds(&mut engine);
        for (case, program, expected) in failing_programs(outside) {
            let err = engine
                .schedule_program_blocks(&program)
                .expect_err("rejected");
            assert_eq!(err.to_string(), expected, "{case}");
        }

        drive(engine, move |mut client| async move {
            assert_eq!(client.save().await.blocks.len(), 2, "nothing was scheduled");
        })
        .await;
    }

    /// A program with a link the actors fail to wire — a link from an
    /// input of a block to another input of the same block passes every
    /// check, but its block rejects it — sorting after a link they wire,
    /// and the ids of both links.
    fn failing_to_wire() -> (Program, Uuid, Uuid, Uuid, Uuid) {
        let (mut program, first, second) = two_adds();
        let (wired, rejected) = ordered_ids();
        let link = program_link(first, second, "in0", Some(wired));
        program.links.insert(wired.to_string(), link);
        let mut fan_out = program_link(first, first, "in1", Some(rejected));
        fan_out.source_block_pin_name = "in0".to_string();
        program.links.insert(rejected.to_string(), fan_out);
        (program, first, second, wired, rejected)
    }

    /// A program the actors fail to wire is removed again, links and
    /// all: the engine saves as before, and the program's ids are free
    /// again.
    pub(crate) async fn a_program_failing_to_wire_is_removed_again<E: TestEngine>(engine: E) {
        let (mut program, first, second, wired, rejected) = failing_to_wire();

        drive(engine, move |mut client| async move {
            let before = json(&client.save().await);
            let reply = client.load(program.clone()).await;
            let err = EngineError::from(ConnectError::SelfLink {
                block: first,
                source_input: "in0".to_string(),
                target_input: "in1".to_string(),
            });
            assert_eq!(reply, Err(err.to_string()));
            assert_eq!(json(&client.save().await), before, "the load was undone");

            program.links.remove(&rejected.to_string());
            client.load(program).await.expect("the ids are free again");
            assert!(client.inspect(second).await.inputs["in0"].is_connected);
            assert_eq!(client.remove_link(wired).await, Ok(true));
        })
        .await;
    }

    /// Scheduled while idle, the same program keeps its blocks — there
    /// is no load to undo by then — and `run` drops the rejected link
    /// alone.
    pub(crate) async fn a_program_scheduled_while_idle_drops_a_link_it_fails_to_wire<
        E: TestEngine,
    >(
        mut engine: E,
    ) {
        let (program, _, second, wired, rejected) = failing_to_wire();
        engine.schedule_program_blocks(&program).expect("scheduled");

        drive(engine, move |mut client| async move {
            let saved = client.save().await;
            assert_eq!(saved.blocks.len(), 2);
            assert_eq!(
                saved.links.into_keys().collect::<Vec<_>>(),
                vec![wired.to_string()]
            );
            assert!(client.inspect(second).await.inputs["in0"].is_connected);
            assert_eq!(client.remove_link(rejected).await, Ok(false));
        })
        .await;
    }

    /// Regression: a program scheduled while idle never got its saved
    /// pin values.
    pub(crate) async fn a_program_scheduled_while_idle_gets_its_pin_values<E: TestEngine>(
        mut engine: E,
    ) {
        let (mut program, first, second) = two_adds();
        let block = program.blocks.get_mut(&first.to_string()).expect("block");
        let value = PinValue {
            value: 4.into(),
            is_connected: false,
        };
        block.inputs.insert("in0".to_string(), value);
        let link = program_link(first, second, "in0", None);
        program.links.insert(Uuid::new_v4().to_string(), link);
        engine.schedule_program_blocks(&program).expect("scheduled");

        drive(engine, move |mut client| async move {
            assert_eq!(client.inspect(first).await.inputs["in0"].val, 4.into());
            client.wait_for_output(second, "out", 4.into()).await;
        })
        .await;
    }
}

/// Blocks whose actor is gone, written once against the [`Engine`]
/// trait and run by each engine's tests: such a block can still be
/// saved, unlinked and removed.
pub(crate) mod dead_actors {
    use assert_matches::assert_matches;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::{Block, BlockState};
    use crate::base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes};
    use crate::base::error::EngineError;
    use crate::base::input::InputReader;
    use crate::blocks::{InputImpl, OutputImpl};

    /// Ends its own actor — by entering [`BlockState::Terminated`] —
    /// once `quit` reads a positive number.
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct Quits {
        #[input(kind = "Number")]
        quit: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }

    impl Block for Quits {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
            if matches!(&self.quit.val, Some(Value::Number(n)) if n.value > 0.0) {
                self.set_state(BlockState::Terminated);
            }
        }
    }

    /// The blocks of an `Add → Quits → Add` chain, and its links.
    struct Chain {
        source: Uuid,
        quits: Uuid,
        target: Uuid,
        into_quits: Uuid,
        out_of_quits: Uuid,
    }

    /// Schedules a chain whose middle block ends its actor once
    /// [`kill`] writes to the source.
    fn schedule_chain<E: TestEngine>(engine: &mut E) -> Chain {
        let mut source = Add::new();
        let mut quits = Quits::new();
        let mut target = Add::new();
        let into_quits = connect_output(&mut source.out, &mut quits.quit).expect("linked");
        let out_of_quits = connect_output(&mut quits.out, target.inputs_mut()[0]).expect("linked");
        let chain = Chain {
            source: *source.id(),
            quits: *quits.id(),
            target: *target.id(),
            into_quits,
            out_of_quits,
        };
        engine.schedule(source).expect("scheduled");
        engine.schedule(quits).expect("scheduled");
        engine.schedule(target).expect("scheduled");
        chain
    }

    /// Ends the chain's middle actor, and waits until it is gone.
    async fn kill(client: &mut Client, chain: &Chain) {
        client.write_output(chain.source, "out", 1.into()).await;
        for _ in 0..POLLS {
            let reply = client
                .request(InspectBlockReq(client.id, chain.quits))
                .await;
            if let InspectBlockRes(Err(msg)) = reply {
                assert_eq!(
                    msg,
                    EngineError::BlockTaskGone { id: chain.quits }.to_string()
                );
                return;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!("the actor never ended");
    }

    /// A block whose actor is gone is saved from its handle — with its
    /// links, but no pin values — instead of failing the save.
    pub(crate) async fn a_block_whose_actor_is_gone_is_saved_without_pins<E: TestEngine>(
        mut engine: E,
    ) {
        let chain = schedule_chain(&mut engine);

        drive(engine, move |mut client| async move {
            kill(&mut client, &chain).await;

            let saved = client.save().await;
            let quits = &saved.blocks[&chain.quits.to_string()];
            assert_eq!(quits.name, "Quits");
            assert!(quits.inputs.is_empty() && quits.outputs.is_empty());
            let mut links: Vec<_> = saved.links.into_keys().collect();
            links.sort();
            let mut expected = vec![chain.into_quits.to_string(), chain.out_of_quits.to_string()];
            expected.sort();
            assert_eq!(links, expected);
        })
        .await;
    }

    /// A link out of a block whose actor is gone can still be removed,
    /// and its target input stops counting it.
    pub(crate) async fn a_link_out_of_a_gone_actor_is_removed<E: TestEngine>(mut engine: E) {
        let chain = schedule_chain(&mut engine);

        drive(engine, move |mut client| async move {
            kill(&mut client, &chain).await;
            assert!(client.inspect(chain.target).await.inputs["in0"].is_connected);

            assert_eq!(client.remove_link(chain.out_of_quits).await, Ok(true));
            assert!(!client.inspect(chain.target).await.inputs["in0"].is_connected);
            assert_eq!(client.remove_link(chain.out_of_quits).await, Ok(false));
        })
        .await;
    }

    /// A block whose actor is gone can still be removed: its handle and
    /// its links go, both ways.
    pub(crate) async fn a_block_whose_actor_is_gone_is_removed<E: TestEngine>(mut engine: E) {
        let chain = schedule_chain(&mut engine);

        drive(engine, move |mut client| async move {
            kill(&mut client, &chain).await;

            assert_eq!(client.remove_block(chain.quits).await, Ok(chain.quits));
            let reply = client
                .request(InspectBlockReq(client.id, chain.quits))
                .await;
            assert_matches!(reply, InspectBlockRes(Err(msg))
                if msg == EngineError::BlockInstanceNotFound { id: chain.quits }.to_string());
            assert!(!client.inspect(chain.target).await.inputs["in0"].is_connected);
            let saved = client.save().await;
            assert_eq!(saved.blocks.len(), 2);
            assert!(saved.links.is_empty(), "both links are gone");
        })
        .await;
    }
}

/// Connector deadlines, written once against the [`Engine`] trait and
/// run by each engine's tests: every engine request addressed to a
/// block cancels its `execute` cycle, yet a slow connector operation
/// still times out on time.
pub(crate) mod connector_deadlines {
    use std::sync::Arc;
    use std::time::Instant;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::connector::{register_connector, unregister_connector};
    use crate::base::program::PinValue;
    use crate::blocks::external::support::mock::MockConnector;
    use crate::blocks::external::{ExternalIn, ExternalOut, Request};

    /// The deadline each slow operation is given, in milliseconds.
    const TIMEOUT_MILLIS: f64 = 150.0;

    /// How long the fake connector takes over each operation: far past
    /// the deadline.
    const DELAY_MILLIS: u64 = 10_000;

    /// How late a timeout may fire: generous for a loaded machine, yet
    /// far short of the operation completing — or of never timing out.
    const SLACK: Duration = Duration::from_secs(2);

    /// How long the late-timeout tests let an operation run under the
    /// default deadline before they write `timeout`: long enough for
    /// the block to have issued it, far short of that deadline.
    const HEAD_START: Duration = Duration::from_millis(50);

    /// Inspects `blocks` every few milliseconds — each inspection
    /// cancelling a block's cycle — until all of them faulted, and
    /// checks each faulted by timing out after [`TIMEOUT_MILLIS`], on
    /// time: within [`SLACK`] of that deadline, counted from `started`.
    async fn wait_for_timeouts(client: &mut Client, blocks: &[Uuid], started: Instant) {
        let deadline = Duration::from_millis(TIMEOUT_MILLIS as u64) + SLACK;
        let mut running = blocks.to_vec();
        while !running.is_empty() {
            assert!(
                started.elapsed() < deadline,
                "the operations of {running:?} never timed out under steady requests"
            );
            let mut still_running = Vec::new();
            for block in running {
                let definition = client.inspect(block).await;
                if definition.state != "fault" {
                    still_running.push(block);
                    continue;
                }
                let reason = definition.fault_reason.unwrap_or_default();
                assert!(
                    reason.contains("timed out after 150ms"),
                    "{block} faulted by its timeout, got: {reason}"
                );
            }
            running = still_running;
            sleep(Duration::from_millis(5)).await;
        }
    }

    /// Writes `pins` to `block`, then checks its operation times out on
    /// time under steady requests.
    async fn times_out_under_requests(client: &mut Client, block: Uuid, pins: Vec<(&str, Value)>) {
        for (pin, value) in pins {
            client.write_input(block, pin, value).await;
        }
        wait_for_timeouts(client, &[block], Instant::now()).await;
    }

    /// Writes `pins` to `block`, which issues its operation under the
    /// default deadline, then — with the operation in flight — writes
    /// `timeout`, the way a program load does, and checks the operation
    /// times out by the timeout written last.
    async fn times_out_by_a_late_timeout(
        client: &mut Client,
        block: Uuid,
        pins: Vec<(&str, Value)>,
    ) {
        let started = Instant::now();
        for (pin, value) in pins {
            client.write_input(block, pin, value).await;
        }
        sleep(HEAD_START).await;
        client
            .write_input(block, "timeout", Value::make_number(TIMEOUT_MILLIS))
            .await;
        wait_for_timeouts(client, &[block], started).await;
    }

    /// Registers a [`MockConnector`] under a fresh name, with `slow`
    /// applied, and returns the name.
    fn register_slow(prefix: &str, slow: impl FnOnce(&mut MockConnector)) -> String {
        let name = format!("{prefix}-{}", Uuid::new_v4());
        let mut connector = MockConnector::default();
        slow(&mut connector);
        register_connector(&name, Arc::new(connector)).expect("registered");
        name
    }

    /// The pins binding an external block to `connector`, its timeout
    /// first, so the deadline is set by the time the operation starts.
    fn binding(connector: &str, address: &str) -> Vec<(&'static str, Value)> {
        vec![
            ("timeout", Value::make_number(TIMEOUT_MILLIS)),
            ("connector", Value::make_str(connector)),
            ("address", Value::make_str(address)),
        ]
    }

    /// A slow publish times out.
    pub(crate) async fn external_out_times_out_under_requests<E: TestEngine>(mut engine: E) {
        let name = register_slow("deadline-out", |mock| {
            mock.publish_delay_millis = Some(DELAY_MILLIS);
        });
        let block = ExternalOut::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let mut pins = binding(&name, "topic");
        pins.push(("in", 42.into()));
        drive(engine, move |mut client| async move {
            times_out_under_requests(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// A slow request times out.
    pub(crate) async fn request_times_out_under_requests<E: TestEngine>(mut engine: E) {
        let name = register_slow("deadline-req", |mock| {
            mock.request_delay_millis = Some(DELAY_MILLIS);
        });
        let block = Request::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let mut pins = binding(&name, "route");
        pins.push(("in", 42.into()));
        drive(engine, move |mut client| async move {
            times_out_under_requests(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// A slow subscribe times out.
    pub(crate) async fn external_in_times_out_under_requests<E: TestEngine>(mut engine: E) {
        let name = register_slow("deadline-in", |mock| {
            mock.subscribe_delay_millis = Some(DELAY_MILLIS);
        });
        let block = ExternalIn::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let pins = binding(&name, "sensor");
        drive(engine, move |mut client| async move {
            times_out_under_requests(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// The pins binding an external block to `connector`, without a
    /// timeout: the default applies.
    fn unbounded_binding(connector: &str, address: &str) -> Vec<(&'static str, Value)> {
        let mut pins = binding(connector, address);
        pins.retain(|(pin, _)| *pin != "timeout");
        pins
    }

    /// A timeout written while a publish is in flight applies to it.
    pub(crate) async fn external_out_applies_a_late_timeout<E: TestEngine>(mut engine: E) {
        let name = register_slow("late-timeout-out", |mock| {
            mock.publish_delay_millis = Some(DELAY_MILLIS);
        });
        let block = ExternalOut::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let mut pins = unbounded_binding(&name, "topic");
        pins.push(("in", 42.into()));
        drive(engine, move |mut client| async move {
            times_out_by_a_late_timeout(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// A timeout written while a request is in flight applies to it.
    pub(crate) async fn request_applies_a_late_timeout<E: TestEngine>(mut engine: E) {
        let name = register_slow("late-timeout-req", |mock| {
            mock.request_delay_millis = Some(DELAY_MILLIS);
        });
        let block = Request::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let mut pins = unbounded_binding(&name, "route");
        pins.push(("in", 42.into()));
        drive(engine, move |mut client| async move {
            times_out_by_a_late_timeout(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// A timeout written while a subscribe is in flight applies to it.
    pub(crate) async fn external_in_applies_a_late_timeout<E: TestEngine>(mut engine: E) {
        let name = register_slow("late-timeout-in", |mock| {
            mock.subscribe_delay_millis = Some(DELAY_MILLIS);
        });
        let block = ExternalIn::new();
        let id = *block.id();
        engine.schedule(block).expect("scheduled");

        let pins = unbounded_binding(&name, "sensor");
        drive(engine, move |mut client| async move {
            times_out_by_a_late_timeout(&mut client, id, pins).await;
        })
        .await;
        unregister_connector(&name);
    }

    /// A loaded program's saved timeouts apply to its blocks' first
    /// operations. A load writes a block's saved values in pin-name
    /// order — `timeout` after the pins that start the operation — and
    /// on the multi-threaded engine the block can run in between.
    pub(crate) async fn a_loaded_timeout_applies_to_the_first_operation<E: TestEngine>(engine: E) {
        let out = register_slow("loaded-timeout-out", |mock| {
            mock.publish_delay_millis = Some(DELAY_MILLIS);
        });
        let req = register_slow("loaded-timeout-req", |mock| {
            mock.request_delay_millis = Some(DELAY_MILLIS);
        });
        let sub = register_slow("loaded-timeout-in", |mock| {
            mock.subscribe_delay_millis = Some(DELAY_MILLIS);
        });

        let mut program = Program::default();
        let mut ids = Vec::new();
        for (name, connector, value) in [
            ("ExternalOut", &out, Some(42)),
            ("Request", &req, Some(42)),
            ("ExternalIn", &sub, None),
        ] {
            let mut pins = binding(connector, "address");
            pins.extend(value.map(|value| ("in", value.into())));
            let inputs = pins
                .into_iter()
                .map(|(pin, value)| {
                    let value = PinValue {
                        value,
                        is_connected: false,
                    };
                    (pin.to_string(), value)
                })
                .collect();
            let block = ProgramBlock {
                name: name.to_string(),
                lib: "core".to_string(),
                inputs,
                ..Default::default()
            };
            let id = Uuid::new_v4();
            program.blocks.insert(id.to_string(), block);
            ids.push(id);
        }

        drive(engine, move |mut client| async move {
            let started = Instant::now();
            client.load(program).await.expect("loaded");
            wait_for_timeouts(&mut client, &ids, started).await;
        })
        .await;
        for name in [out, req, sub] {
            unregister_connector(&name);
        }
    }
}

/// Panic isolation, written once against the [`Engine`] trait and run by
/// each engine's tests. See `actor_step` for the contract.
pub(crate) mod panics {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use assert_matches::assert_matches;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::Status;
    use crate::base::block::Block;
    use crate::base::engine::messages::EngineMessage::{
        GetCurrentProgramReq, GetCurrentProgramRes, InspectBlockReq, InspectBlockRes,
        RemoveBlockReq, RemoveBlockRes,
    };
    use crate::base::input::InputReader;
    use crate::base::output::Output;
    use crate::blocks::{InputImpl, OutputImpl};
    use crate::tokio_impl::engine::actor_step::PANIC_RETRY_INITIAL;

    /// The fault reason a [`PanicOnTrigger`] panic leaves.
    const REASON: &str = "block panicked: trigger is set";

    /// Panics whenever an execute cycle reads a positive `trigger`.
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct PanicOnTrigger {
        #[input(kind = "Number")]
        trigger: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
        /// Counts the panics.
        panics: Arc<AtomicUsize>,
    }

    impl Block for PanicOnTrigger {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
            if matches!(&self.trigger.val, Some(Value::Number(n)) if n.value > 0.0) {
                self.panics.fetch_add(1, Ordering::SeqCst);
                panic!("trigger is set");
            }
        }
    }

    /// Reports in `fault` whether the last value on `source` arrived
    /// with [`Status::Fault`].
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct FaultProbe {
        #[input(kind = "Number")]
        source: InputImpl,
        #[output(kind = "Bool")]
        fault: OutputImpl,
    }

    impl Block for FaultProbe {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
            let fault = self.source.status == Status::Fault;
            self.fault.set(fault.into());
        }
    }

    /// The ids of a `source → panicker → probe` chain, and the
    /// panicker's panic count.
    struct Chain {
        source: Uuid,
        panicker: Uuid,
        probe: Uuid,
        panics: Arc<AtomicUsize>,
    }

    /// Schedules an `Add` whose `out` feeds a [`PanicOnTrigger`]'s
    /// `trigger`, whose `out` feeds a [`FaultProbe`].
    fn schedule_chain<E: TestEngine>(engine: &mut E) -> Chain {
        let mut source = Add::new();
        let mut panicker = PanicOnTrigger::new();
        let mut probe = FaultProbe::new();
        connect_output(&mut source.out, &mut panicker.trigger).expect("linked");
        connect_output(&mut panicker.out, &mut probe.source).expect("linked");

        let chain = Chain {
            source: *source.id(),
            panicker: *panicker.id(),
            probe: *probe.id(),
            panics: panicker.panics.clone(),
        };
        engine.schedule(source).expect("source scheduled");
        engine.schedule(panicker).expect("panicker scheduled");
        engine.schedule(probe).expect("probe scheduled");
        chain
    }

    /// A panic faults its block and the blocks downstream of it, while
    /// the block stays inspectable, savable and removable, and is not
    /// retried in a hot loop.
    pub(crate) async fn panic_faults_the_block_and_downstream<E: TestEngine>(mut engine: E) {
        let chain = schedule_chain(&mut engine);

        drive(engine, move |mut client| async move {
            client.write_output(chain.source, "out", 1.into()).await;

            let panicked = client.wait_for_state(chain.panicker, "fault").await;
            assert_eq!(panicked.fault_reason.as_deref(), Some(REASON));
            client
                .wait_for_output(chain.probe, "fault", true.into())
                .await;

            match client.request(GetCurrentProgramReq(client.id)).await {
                GetCurrentProgramRes(Ok(program)) => assert_eq!(program.blocks.len(), 3),
                other => panic!("Expected GetCurrentProgramRes(Ok), got {:?}", other),
            }

            // Not repeated, since no new trigger value arrives; the hold
            // for a block that panics on every cycle is covered by
            // `a_block_panicking_every_cycle_backs_off`.
            sleep(Duration::from_millis(200)).await;
            assert_eq!(chain.panics.load(Ordering::SeqCst), 1);
            assert_eq!(client.inspect(chain.panicker).await.state, "fault");

            let reply = client
                .request(RemoveBlockReq(client.id, chain.panicker))
                .await;
            assert_matches!(reply, RemoveBlockRes(Ok(id)) if id == chain.panicker);
            let reply = client
                .request(InspectBlockReq(client.id, chain.panicker))
                .await;
            assert_matches!(reply, InspectBlockRes(Err(_)), "the block was removed");
        })
        .await;
    }

    /// After the retry delay a held block runs again, and a clean cycle
    /// ends the fault — upstream of the block and downstream of it.
    pub(crate) async fn panicked_block_recovers_after_a_clean_cycle<E: TestEngine>(mut engine: E) {
        let chain = schedule_chain(&mut engine);

        drive(engine, move |mut client| async move {
            client.write_output(chain.source, "out", 1.into()).await;
            client.wait_for_state(chain.panicker, "fault").await;
            client
                .wait_for_output(chain.probe, "fault", true.into())
                .await;

            client.write_output(chain.source, "out", 0.into()).await;
            client.wait_for_state(chain.panicker, "running").await;
            client
                .wait_for_output(chain.probe, "fault", false.into())
                .await;
            assert_eq!(chain.panics.load(Ordering::SeqCst), 1);
        })
        .await;
    }

    /// Panics on every execute cycle, without waiting for anything.
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct AlwaysPanic {
        #[output(kind = "Number")]
        out: OutputImpl,
        /// Counts the panics.
        panics: Arc<AtomicUsize>,
    }

    impl Block for AlwaysPanic {
        async fn execute(&mut self) {
            self.panics.fetch_add(1, Ordering::SeqCst);
            panic!("always");
        }
    }

    /// A block that panics on every cycle is held between retries, with
    /// the delay doubling each time, rather than re-run in a hot loop.
    pub(crate) async fn a_block_panicking_every_cycle_backs_off<E: TestEngine>(mut engine: E) {
        let block = AlwaysPanic::new();
        let (id, panics) = (*block.id(), block.panics.clone());
        engine.schedule(block).expect("scheduled");

        drive(engine, move |mut client| async move {
            let held = client.wait_for_state(id, "fault").await;
            assert_eq!(held.fault_reason.as_deref(), Some("block panicked: always"));

            sleep(PANIC_RETRY_INITIAL / 4).await;
            assert_eq!(
                panics.load(Ordering::SeqCst),
                1,
                "held for the first retry delay, not re-run at once"
            );

            // Retried once the first delay runs out, then held twice as
            // long: still one retry after one and a half first delays.
            wait_for_panics(&panics, 2).await;
            sleep(PANIC_RETRY_INITIAL * 3 / 2).await;
            assert_eq!(
                panics.load(Ordering::SeqCst),
                2,
                "the second retry delay is doubled"
            );
            assert_eq!(client.inspect(id).await.state, "fault");
        })
        .await;
    }

    /// Waits until `panics` reaches `count`, failing the test after a few
    /// seconds.
    async fn wait_for_panics(panics: &AtomicUsize, count: usize) {
        for _ in 0..POLLS {
            if panics.load(Ordering::SeqCst) >= count {
                return;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!("the block never panicked {count} times");
    }
}

/// Actor task lifetime, written once against the [`Engine`] trait and
/// run by each engine's tests: dropping an engine ends every actor task.
pub(crate) mod drops {
    use std::sync::Arc;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::Block;
    use crate::base::input::InputReader;
    use crate::blocks::OutputImpl;

    /// How many blocks each test schedules.
    const BLOCKS: usize = 4;

    /// A block whose only job is to count its live instances: every
    /// instance holds a clone of `alive`, and an actor task drops its
    /// block when it ends.
    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    pub(crate) struct Tracked {
        #[output(kind = "Number")]
        out: OutputImpl,
        alive: Arc<()>,
    }

    impl Block for Tracked {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
        }
    }

    /// Schedules [`BLOCKS`] tracked blocks and returns their counter.
    pub(crate) fn schedule_tracked<E: TestEngine>(engine: &mut E) -> Arc<()> {
        let alive = Arc::new(());
        for _ in 0..BLOCKS {
            let mut block = Tracked::new();
            block.alive = alive.clone();
            engine.schedule(block).expect("scheduled");
        }
        assert_eq!(Arc::strong_count(&alive), BLOCKS + 1);
        alive
    }

    /// Waits until every tracked block was dropped — that is, every
    /// actor task has ended — failing the test after a few seconds.
    pub(crate) async fn wait_for_actors_to_end(alive: &Arc<()>) {
        for _ in 0..POLLS {
            if Arc::strong_count(alive) == 1 {
                return;
            }
            sleep(POLL_INTERVAL).await;
        }
        panic!(
            "{} actor task(s) outlived their engine",
            Arc::strong_count(alive) - 1
        );
    }

    /// An engine dropped before it ever ran.
    pub(crate) async fn dropping_an_idle_engine_ends_its_actors<E: TestEngine>(mut engine: E) {
        let alive = schedule_tracked(&mut engine);
        drop(engine);
        wait_for_actors_to_end(&alive).await;
    }

    /// An engine dropped after `run()` handed it back.
    pub(crate) async fn dropping_a_shut_down_engine_ends_its_actors<E: TestEngine>(mut engine: E) {
        let alive = schedule_tracked(&mut engine);
        let engine = drive(engine, |_| async {}).await;
        drop(engine);
        wait_for_actors_to_end(&alive).await;
    }

    /// An engine dropped together with its unfinished `run()` future.
    pub(crate) async fn dropping_the_run_future_ends_its_actors<E: TestEngine>(mut engine: E) {
        let alive = schedule_tracked(&mut engine);
        let unfinished = timeout(Duration::from_millis(100), engine.run()).await;
        assert!(unfinished.is_err(), "run() only ends on Shutdown");
        wait_for_actors_to_end(&alive).await;
    }
}

/// Blocks without a static description, written once against the
/// [`Engine`] trait and run by each engine's tests: a block whose
/// description only exists at runtime, like a JS block's, and a
/// runtime-registered block, which the registry type-erases, both
/// schedule, connect, save and load without implementing
/// [`BlockStaticDesc`](crate::base::block::BlockStaticDesc).
pub(crate) mod runtime_descs {
    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::{Block, BlockConnect, BlockDesc, BlockInput, BlockOutput, BlockState};
    use crate::base::engine::messages::EngineMessage::Reset;
    use crate::base::input::{InputProps, input_reader::InputReader};
    use crate::base::link::Link;
    use crate::base::output::{Output, OutputProps};
    use crate::blocks::{InputImpl, OutputImpl, registry};
    use libhaystack::val::kind::HaystackKind;

    /// The library the blocks of these tests are described in.
    const LIBRARY: &str = "runtime_desc_test";

    /// Copies `in` to `out`. Implements [`BlockProps`] and [`Block`] by
    /// hand around a description built at runtime, and nothing else.
    #[derive(Debug)]
    pub(crate) struct Echo {
        id: Uuid,
        desc: BlockDesc,
        input: InputImpl,
        out: OutputImpl,
        state: BlockState,
    }

    impl Echo {
        /// A new `Echo`, described as `runtime_desc_test::Echo`.
        pub(crate) fn new() -> Self {
            let id = Uuid::new_v4();
            let desc = BlockDesc::new("Echo", LIBRARY)
                .with_input("in", HaystackKind::Number)
                .with_output("out", HaystackKind::Number);
            Self {
                id,
                desc,
                input: InputImpl::new("in", HaystackKind::Number, id),
                out: OutputImpl::new_named("out", HaystackKind::Number, id),
                state: BlockState::Running,
            }
        }
    }

    impl BlockProps for Echo {
        type Reader = ReaderImpl;
        type Writer = WriterImpl;

        fn id(&self) -> &Uuid {
            &self.id
        }

        fn name(&self) -> &str {
            &self.desc.name
        }

        fn desc(&self) -> &BlockDesc {
            &self.desc
        }

        fn state(&self) -> BlockState {
            self.state.clone()
        }

        fn set_state(&mut self, state: BlockState) -> BlockState {
            self.state = state;
            self.state.clone()
        }

        fn inputs(&self) -> Vec<&(dyn BlockInput<ReaderImpl, WriterImpl> + Send)> {
            vec![&self.input]
        }

        fn inputs_mut(&mut self) -> Vec<&mut (dyn BlockInput<ReaderImpl, WriterImpl> + Send)> {
            vec![&mut self.input]
        }

        fn outputs(&self) -> Vec<&(dyn BlockOutput<WriterImpl> + Send)> {
            vec![&self.out]
        }

        fn outputs_mut(&mut self) -> Vec<&mut (dyn BlockOutput<WriterImpl> + Send)> {
            vec![&mut self.out]
        }

        fn links(&self) -> Vec<(&str, Vec<&(dyn Link + Send)>)> {
            vec![
                (self.input.name(), self.input.links()),
                (self.out.name(), self.out.links()),
            ]
        }

        fn remove_link_by_id(&mut self, link_id: &Uuid) {
            self.input.remove_link_by_id(link_id);
            self.out.remove_link_by_id(link_id);
        }

        fn remove_all_links(&mut self) {
            self.input.remove_all_links();
            self.out.remove_all_links();
        }
    }

    impl Block for Echo {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
            if let Some(value) = self.input.value().cloned() {
                self.out.set(value);
            }
        }
    }

    /// Runtime-registered as `runtime_desc_test::Registered`: copies
    /// `in` to `out`.
    #[block]
    #[derive(BlockProps, Debug)]
    #[library = "runtime_desc_test"]
    #[category = "test"]
    pub(crate) struct Registered {
        #[input(name = "in", kind = "Number")]
        input: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }

    impl Block for Registered {
        async fn execute(&mut self) {
            self.read_inputs_until_ready().await;
            if let Some(value) = self.input.value().cloned() {
                self.out.set(value);
            }
        }
    }

    /// A block with a runtime description connects through
    /// [`BlockConnect`] and through the engine, runs, and saves under
    /// the name and library its description gives.
    pub(crate) async fn a_block_with_a_runtime_desc_runs_and_saves<E: TestEngine>(mut engine: E) {
        let mut first = Echo::new();
        let mut second = Echo::new();
        let third = Echo::new();
        let wired = first
            .connect_output("out", second.inputs_mut()[0])
            .expect("linked by name");
        let ids = (*first.id(), *second.id(), *third.id());
        engine.schedule(first).expect("scheduled");
        engine.schedule(second).expect("scheduled");
        engine.schedule(third).expect("scheduled");

        drive(engine, move |mut client| async move {
            let (first, second, third) = ids;
            let link = client
                .connect(None, (second, "out"), (third, "in"))
                .await
                .expect("linked by the engine");

            client.write_input(first, "in", 7.into()).await;
            client.wait_for_output(third, "out", 7.into()).await;

            let saved = client.save().await;
            for id in [first, second, third] {
                let block = &saved.blocks[&id.to_string()];
                assert_eq!((block.name.as_str(), block.lib.as_str()), ("Echo", LIBRARY));
            }
            let mut links: Vec<_> = saved.links.into_keys().collect();
            links.sort();
            let mut expected = vec![wired.to_string(), link.id.expect("an id")];
            expected.sort();
            assert_eq!(links, expected);
        })
        .await;
    }

    /// Regression: a request that named no library looked the block up
    /// in the core library only, so a block registered in its own
    /// library could not be added or evaluated by its name alone, while
    /// `schedule_block` with no library searched every library.
    pub(crate) async fn a_registered_block_is_found_by_its_name_alone<E: TestEngine>(engine: E) {
        use crate::base::engine::messages::EngineMessage::{
            AddBlockReq, AddBlockRes, EvaluateBlockReq, EvaluateBlockRes,
        };

        let _ = registry::register::<Registered>();
        drive(engine, move |mut client| async move {
            let added = client
                .request(AddBlockReq(client.id, "Registered".to_string(), None, None))
                .await;
            let AddBlockRes(Ok(id)) = added else {
                panic!("not added: {added:?}");
            };
            assert_eq!(client.inspect(id).await.name, "Registered");

            let evaluated = client
                .request(EvaluateBlockReq(
                    client.id,
                    "Registered".to_string(),
                    vec![5.into()],
                    None,
                ))
                .await;
            let EvaluateBlockRes(Ok(outputs)) = evaluated else {
                panic!("not evaluated: {evaluated:?}");
            };
            assert_eq!(outputs, vec![Value::from(5)]);
        })
        .await;
    }

    /// A program of runtime-registered blocks, which the engine
    /// schedules as the registry's type-erased block, loads, runs, and
    /// saves back as it was loaded.
    pub(crate) async fn a_registered_block_loads_and_saves<E: TestEngine>(engine: E) {
        let _ = registry::register::<Registered>();
        let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
        let mut program = add_program([first.to_string().as_str(), &second.to_string()]);
        for block in program.blocks.values_mut() {
            block.name = "Registered".to_string();
            block.lib = LIBRARY.to_string();
        }
        let link = Uuid::new_v4();
        program.links.insert(
            link.to_string(),
            program_link(first, second, "in", Some(link)),
        );

        drive(engine, move |mut client| async move {
            client.load(program).await.expect("loaded");
            client.write_input(first, "in", 3.into()).await;
            client.wait_for_output(second, "out", 3.into()).await;

            let saved = client.save().await;
            assert_eq!(saved.blocks[&second.to_string()].name, "Registered");
            assert!(saved.links.contains_key(&link.to_string()));

            client.send(Reset).await;
            client.load(saved.clone()).await.expect("reloaded");
            super::round_trip::wait_for_save(&mut client, &saved).await;
        })
        .await;
    }
}

/// Per-engine scan intervals, written once against the [`Engine`]
/// trait and run by each engine's tests.
pub(crate) mod scan_intervals {
    use std::time::Duration;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::block::Block;
    use crate::base::input::InputReader;
    use crate::base::output::Output;
    use crate::blocks::OutputImpl;
    use crate::tokio_impl::scan::scan_interval_millis;

    /// Publishes the scan interval its actor task runs with on `out`.
    #[block]
    #[derive(BlockProps, Debug)]
    #[library = "scan_interval_test"]
    #[category = "test"]
    pub(crate) struct ScanProbe {
        #[output(kind = "Number")]
        out: OutputImpl,
    }

    impl Block for ScanProbe {
        async fn execute(&mut self) {
            self.out.set((scan_interval_millis() as f64).into());
            // No linked inputs: this waits a scan interval or more.
            self.read_inputs_until_ready().await;
        }
    }

    /// Schedules a [`ScanProbe`] on `engine` and returns the interval,
    /// in milliseconds, it reports once the engine runs.
    async fn probe<E: TestEngine>(mut engine: E, expected: f64) {
        let probe = ScanProbe::new();
        let id = *probe.id();
        engine.schedule(probe).expect("scheduled");
        drive(engine, move |mut client| async move {
            client.wait_for_output(id, "out", expected.into()).await;
        })
        .await;
    }

    /// Regression: the scan interval was one process-wide value, so
    /// configuring a second engine changed it for the first one's
    /// blocks too.
    pub(crate) async fn each_engine_keeps_its_own_scan_interval<E: TestEngine>(
        first: E,
        second: E,
    ) {
        let first = first.with_scan_interval(Duration::from_millis(40));
        let second = second.with_scan_interval(Duration::from_millis(60));
        assert_eq!(first.scan_interval(), Duration::from_millis(40));

        probe(first, 40.0).await;
        probe(second, 60.0).await;
    }

    /// A new interval reaches the blocks scheduled before it was set.
    pub(crate) async fn a_new_scan_interval_reaches_scheduled_blocks<E: TestEngine>(mut engine: E) {
        let probe = ScanProbe::new();
        let id = *probe.id();
        engine.schedule(probe).expect("scheduled");
        engine.set_scan_interval(Duration::from_millis(30));

        drive(engine, move |mut client| async move {
            client.wait_for_output(id, "out", 30.0.into()).await;
        })
        .await;
    }
}

/// Engine configuration written once against the [`Engine`] trait and
/// run by each engine's tests.
pub(crate) mod configuration {
    use std::sync::Arc;

    use assert_matches::assert_matches;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::program::{EditorMetadata, Position};
    use crate::blocks::external::support::mock::MockConnector;
    use crate::{EngineError, Error};

    /// Regression: `add_connector`, `connector_names`, `block_handles`
    /// and `block_handle` were inherent methods of each engine, and
    /// `set_block_metadata` existed on the single-threaded engine only,
    /// so code generic over [`Engine`] could not configure an engine.
    pub(crate) async fn an_engine_is_configured_through_the_trait<E: TestEngine>(mut engine: E) {
        let name = format!("trait-config-{}", Uuid::new_v4());
        engine
            .add_connector(&name, Arc::new(MockConnector::default()))
            .expect("added");
        assert_eq!(engine.connector_names(), [name]);

        let (source, target) = schedule_linked_adds(&mut engine);
        let mut scheduled: Vec<_> = engine.block_handles().iter().map(|h| *h.id()).collect();
        scheduled.sort();
        let mut expected = vec![source, target];
        expected.sort();
        assert_eq!(scheduled, expected);

        let position = Position { x: 1.0, y: 2.0 };
        engine
            .set_block_metadata(&source, Some("source".to_string()), Some(position))
            .expect("scheduled");
        let handle = engine.block_handle(&source).expect("scheduled");
        assert_eq!(handle.label(), Some("source"));
        assert_eq!(handle.position(), Some(position));

        let unknown = Uuid::new_v4();
        assert!(engine.block_handle(&unknown).is_none());
        assert_matches!(
            engine.set_block_metadata(&unknown, None, None),
            Err(Error::Engine(EngineError::BlockInstanceNotFound { id })) if id == unknown
        );

        drive(engine, move |mut client| async move {
            let saved = client.save().await;
            let block = &saved.blocks[&source.to_string()];
            assert_eq!(block.label.as_deref(), Some("source"));
            assert_eq!(block.position, Some(position));
        })
        .await;
    }

    /// Regression: editor metadata reached an engine only with a loaded
    /// program, so a block scheduled any other way was always saved
    /// without it, and a loaded block's could not change.
    pub(crate) async fn editor_metadata_is_set_through_the_trait<E: TestEngine>(mut engine: E) {
        let mut editor = EditorMetadata::new();
        editor.insert(
            "widget".to_string(),
            serde_json::json!({"kind": "Slider", "config": {"step": 0.5, "series": [null]}}),
        );
        let (source, target) = schedule_linked_adds(&mut engine);
        engine
            .set_block_editor(&source, Some(editor.clone()))
            .expect("scheduled");
        let handle = engine.block_handle(&source).expect("scheduled");
        assert_eq!(handle.editor(), Some(&editor));

        let loaded = Uuid::new_v4();
        let mut program = add_program([loaded.to_string().as_str()]);
        for block in program.blocks.values_mut() {
            block.editor = Some(editor.clone());
        }
        engine.schedule_program_blocks(&program).expect("loads");
        engine.set_block_editor(&loaded, None).expect("scheduled");
        assert_eq!(engine.block_handle(&loaded).expect("loaded").editor(), None);

        let unknown = Uuid::new_v4();
        assert_matches!(
            engine.set_block_editor(&unknown, Some(editor.clone())),
            Err(Error::Engine(EngineError::BlockInstanceNotFound { id })) if id == unknown
        );

        drive(engine, move |mut client| async move {
            let saved = client.save().await;
            assert_eq!(saved.blocks[&source.to_string()].editor, Some(editor));
            assert_eq!(saved.blocks[&target.to_string()].editor, None);
            assert_eq!(saved.blocks[&loaded.to_string()].editor, None);
        })
        .await;
    }
}

/// Rejected pin requests, written once against the [`Engine`] trait and
/// run by each engine's tests: a block that rejects a request reports
/// what it rejected as a typed error, not a bare message.
pub(crate) mod rejections {
    use assert_matches::assert_matches;

    use super::lifecycle::TestEngine;
    use super::*;
    use crate::base::error::{ConnectError, EngineError};

    /// A write to a pin the block does not have names the block and the
    /// pin.
    pub(crate) async fn writing_a_missing_pin_names_it<E: TestEngine>(mut engine: E) {
        let add = Add::new();
        let id = *add.id();
        engine.schedule(add).expect("scheduled");

        drive(engine, move |mut client| async move {
            let request =
                EngineMessage::WriteBlockInputReq(client.id, id, "nope".to_string(), 1.into());
            let expected = EngineError::InputNotFound {
                block: id,
                pin: "nope".to_string(),
            };
            assert_matches!(
                client.request(request).await,
                EngineMessage::WriteBlockInputRes(Err(msg)) if msg == expected.to_string()
            );

            let request =
                EngineMessage::WriteBlockOutputReq(client.id, id, "nope".to_string(), 1.into());
            let expected = EngineError::OutputNotFound {
                block: id,
                pin: "nope".to_string(),
            };
            assert_matches!(
                client.request(request).await,
                EngineMessage::WriteBlockOutputRes(Err(msg)) if msg == expected.to_string()
            );
        })
        .await;
    }

    /// Linking the same two pins twice is rejected as the connect error
    /// the source block raised, naming both pins, and leaves the first
    /// link in place.
    pub(crate) async fn linking_the_same_pins_twice_is_a_connect_error<E: TestEngine>(
        mut engine: E,
    ) {
        let (source, target) = schedule_linked_adds(&mut engine);

        drive(engine, move |mut client| async move {
            let reply = client.connect(None, (source, "out"), (target, "in0")).await;
            let expected = EngineError::Connect(ConnectError::AlreadyConnected {
                source_block: source,
                source_pin: "out".to_string(),
                target_block: target,
                target_input: "in0".to_string(),
            });
            assert_matches!(reply, Err(msg) if msg == expected.to_string());
            assert_eq!(client.save().await.links.len(), 1);
        })
        .await;
    }
}
