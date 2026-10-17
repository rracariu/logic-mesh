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
    use crate::base::input::input_reader::InputReader;
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
    use crate::base::input::input_reader::InputReader;
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
