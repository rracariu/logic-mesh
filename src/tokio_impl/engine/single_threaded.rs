// Copyright (c) 2022-2026, Radu Racariu.

//! Single-threaded engine.
//!
//! ## Architecture
//!
//! Each scheduled block is hosted by its own task on a tokio [`LocalSet`](tokio::task::LocalSet).
//! The block is **owned by value** by that task — there is no
//! [`UnsafeCell`](std::cell::UnsafeCell), no [`Rc`](std::rc::Rc)-shared
//! mutable state, and no aliasing. Every external operation
//! against a block (write input/output, inspect, wire/teardown links) is
//! routed through a per-block `mpsc` mailbox (`BlockMailboxCmd`).
//!
//! The per-block task loop uses [`tokio::select!`] to interleave the block's
//! `execute()` future with mailbox handling. When a mailbox command arrives
//! while `execute()` is suspended, the in-flight future is dropped and the
//! command is handled immediately, so external commands (UI input writes,
//! link rewiring) do not have to wait a full polling cycle.
//!
//! ### Cancellation safety
//!
//! Dropping `block.execute()` mid-poll is safe in this codebase because
//! every block in the standard library structures `execute` as
//! "`wait_on_inputs` (or `read_inputs_until_ready`) followed by purely
//! synchronous logic". The only awaits are at the input-wait stage, and
//! the watch-channel reads we use there don't lose values when cancelled
//! (a partially-awaited `changed()` keeps the value pending; the next
//! cycle drains it from `try_take`).
//!
//! Blocks with a second await after the input wait — the external
//! blocks awaiting a connector operation
//! ([`ExternalOut`](crate::blocks::external::ExternalOut),
//! [`Request`](crate::blocks::external::Request)) and, on wasm, the
//! `JsBlock` awaiting a JS Promise — hold the drained work in a pending
//! slot on the block that survives the drop: the next cycle re-issues
//! it instead of waiting for fresh input, so a cancellation costs a
//! possible duplicate of the external effect (at-least-once), never the
//! reaction itself.
//!
//! ## Module layout
//!
//! - `mailbox` — the `BlockMailboxCmd` enum and the actor-side
//!   `handle_cmd` that turns a command into mutations on the owned block.
//! - `actor` — the per-block task loop (`block_actor_task`) and the
//!   change-of-value detector that runs after each step.
//! - [`engine`] — [`SingleThreadedEngine`] itself: lifecycle, the [`Engine`]
//!   trait impl, sync configuration helpers, and the async APIs that
//!   `message_dispatch` routes external requests to.
//!
//! [`Engine`]: crate::base::engine::Engine

mod actor;
mod engine;

pub use engine::{BlockHandle, Messages, SingleThreadedEngine};

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::base;
    use crate::base::program::{
        Program, ProgramBlock,
        data::{LinkData, PinValue, Position},
    };
    use crate::blocks::{math::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{
        GetCurrentProgramReq, GetCurrentProgramRes, InspectBlockReq, InspectBlockRes,
        LoadProgramReq, LoadProgramRes, Shutdown,
    };

    use super::SingleThreadedEngine;
    use base::engine::Engine;
    use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
    use uuid::Uuid;

    /// Link validation reports which half of the link was wrong, as a
    /// variant carrying the offending id or pin name — no message
    /// parsing needed to tell the cases apart.
    #[tokio::test(flavor = "current_thread")]
    async fn link_validation_reports_matchable_variants() {
        use crate::base::error::{EngineError, Error, LinkEnd};
        use assert_matches::assert_matches;

        let mut eng = SingleThreadedEngine::new();
        let add = Add::new();
        let add_uuid = *add.id();
        eng.schedule(add).expect("scheduled");

        let link = |source: String, target: String, pin: &str| LinkData {
            id: None,
            source_block_uuid: source,
            target_block_uuid: target,
            source_block_pin_name: "out".to_string(),
            target_block_pin_name: pin.to_string(),
        };

        let missing = Uuid::new_v4();
        let err = eng
            .connect_blocks_sync(&link(missing.to_string(), add_uuid.to_string(), "in0"))
            .expect_err("unknown source block is rejected");
        assert_matches!(
            err,
            Error::Engine(EngineError::BlockInstanceNotFound { id }) if id == missing
        );

        let err = eng
            .connect_blocks_sync(&link(
                add_uuid.to_string(),
                add_uuid.to_string(),
                "no_such_pin",
            ))
            .expect_err("unknown target pin is rejected");
        assert_matches!(
            err,
            Error::Engine(EngineError::PinNotFound { end, block, pin })
                if end == LinkEnd::Target && block == add_uuid && pin == "no_such_pin"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn engine_test() {
        use crate::base::block::connect::connect_output;

        let mut add1 = Add::new();
        let add_uuid = *add1.id();

        let mut sine1 = SineWave::new();

        sine1.amplitude.val = Some(3.into());
        sine1.freq.val = Some(200.into());
        connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

        let mut sine2 = SineWave::new();
        sine2.amplitude.val = Some(7.into());
        sine2.freq.val = Some(400.into());

        sine2
            .connect_output("out", add1.inputs_mut()[1])
            .expect("Connected");

        let mut eng = SingleThreadedEngine::new();

        let (sender, mut receiver) = mpsc::channel(32);
        let channel_id = Uuid::new_v4();
        let engine_sender = eng.create_message_channel(channel_id, sender.clone());

        thread::spawn(move || {
            let rt = Runtime::new().expect("RT");

            let handle = rt.spawn(async move {
                sleep(Duration::from_millis(300)).await;

                let _ = engine_sender
                    .send(InspectBlockReq(channel_id, add_uuid))
                    .await;

                let res = receiver.recv().await;

                if let Some(InspectBlockRes(Ok(data))) = res {
                    assert_eq!(data.id, add_uuid.to_string());
                    assert_eq!(data.name, "Add");
                    assert_eq!(data.inputs.len(), 16);
                    assert_eq!(data.outputs.len(), 1);
                } else {
                    panic!("Failed to find block: {:?}", res)
                }

                let _ = engine_sender.send(Shutdown).await;
            });

            rt.block_on(handle)
        });

        eng.schedule(add1).unwrap();
        eng.schedule(sine1).unwrap();
        eng.schedule(sine2).unwrap();

        eng.run().await;
    }

    /// End-to-end Q4+A3 check: a [`Program`] with blocks, a link, an
    /// input constant, and UI metadata (label + position) round-trips
    /// through `LoadProgramReq` and `GetCurrentProgramReq` without the
    /// JS layer being involved at all.
    #[tokio::test(flavor = "current_thread")]
    async fn program_load_save_round_trip() {
        // Two Add blocks wired together. `Add` has 16 numeric inputs +
        // one numeric output; we'll seed `in0` on the first block with
        // a constant.
        let add0_uuid = Uuid::new_v4();
        let add1_uuid = Uuid::new_v4();
        let link_uuid = Uuid::new_v4();

        let mut input_consts = std::collections::BTreeMap::new();
        input_consts.insert(
            "in0".to_string(),
            PinValue {
                value: 42.into(),
                is_connected: false,
            },
        );

        let mut blocks = std::collections::BTreeMap::new();
        blocks.insert(
            add0_uuid.to_string(),
            ProgramBlock {
                name: "Add".to_string(),
                lib: "core".to_string(),
                label: Some("Summer A".to_string()),
                positions: Some(Position { x: 10.0, y: 20.0 }),
                inputs: input_consts,
                outputs: Default::default(),
            },
        );
        blocks.insert(
            add1_uuid.to_string(),
            ProgramBlock {
                name: "Add".to_string(),
                lib: "core".to_string(),
                label: Some("Summer B".to_string()),
                positions: Some(Position { x: 200.0, y: 20.0 }),
                inputs: Default::default(),
                outputs: Default::default(),
            },
        );

        let mut links = std::collections::BTreeMap::new();
        links.insert(
            link_uuid.to_string(),
            LinkData {
                id: Some(link_uuid.to_string()),
                source_block_uuid: add0_uuid.to_string(),
                target_block_uuid: add1_uuid.to_string(),
                source_block_pin_name: "out".to_string(),
                target_block_pin_name: "in0".to_string(),
            },
        );

        let program = Program {
            name: Some("rust-headless-test".to_string()),
            description: None,
            blocks,
            links,
        };

        let mut eng = SingleThreadedEngine::new();
        let (sender, mut receiver) = mpsc::channel(32);
        let channel_id = Uuid::new_v4();
        let engine_sender = eng.create_message_channel(channel_id, sender.clone());

        // Driver thread: pushes Load, reads back via Get, asserts, then
        // shuts down so eng.run() unblocks.
        thread::spawn(move || {
            let rt = Runtime::new().expect("RT");
            let handle = rt.spawn(async move {
                sleep(Duration::from_millis(100)).await;

                let _ = engine_sender
                    .send(LoadProgramReq(channel_id, program.clone()))
                    .await;

                match receiver.recv().await {
                    Some(LoadProgramRes(Ok(()))) => {}
                    other => panic!("Expected LoadProgramRes(Ok), got {:?}", other),
                }

                let _ = engine_sender.send(GetCurrentProgramReq(channel_id)).await;
                let res = receiver.recv().await;
                match res {
                    Some(GetCurrentProgramRes(Ok(saved))) => {
                        assert_eq!(saved.blocks.len(), 2);
                        let saved0 = saved
                            .blocks
                            .get(&add0_uuid.to_string())
                            .expect("first block round-trips");
                        assert_eq!(saved0.name, "Add");
                        assert_eq!(saved0.label.as_deref(), Some("Summer A"));
                        let pos = saved0.positions.expect("position survives");
                        assert!((pos.x - 10.0).abs() < f64::EPSILON);
                        // Input constant round-trips.
                        let in0 = saved0.inputs.get("in0").expect("in0 survives");
                        assert_eq!(in0.value, 42.into());

                        // The wired link survives. After connect, target
                        // input 'in0' on the second block should report
                        // is_connected = true.
                        assert_eq!(saved.links.len(), 1);
                        let saved1 = saved
                            .blocks
                            .get(&add1_uuid.to_string())
                            .expect("second block round-trips");
                        let target_in0 = saved1.inputs.get("in0").expect("target input present");
                        assert!(
                            target_in0.is_connected,
                            "wired target input should report is_connected=true"
                        );
                    }
                    other => panic!("Expected GetCurrentProgramRes(Ok), got {:?}", other),
                }

                let _ = engine_sender.send(Shutdown).await;
            });
            rt.block_on(handle)
        });

        eng.run().await;
    }

    mod external_blocks {
        use std::sync::Arc;

        use super::*;
        use crate::base::connector::unregister_connector;
        use crate::base::engine::messages::EngineMessage::{
            WriteBlockInputReq, WriteBlockInputRes,
        };
        use crate::blocks::external::support::mock::MockConnector;
        use crate::blocks::external::{ExternalIn, ExternalOut, Request};
        use libhaystack::val::Value;

        /// End-to-end: a [`MockConnector`] feeds an [`ExternalIn`] block
        /// whose output is wired to an `Add` block — external data flows
        /// through the connector subscription into the pin graph.
        #[tokio::test(flavor = "current_thread")]
        async fn external_in_feeds_add() {
            use crate::base::block::connect::connect_output;

            let name = format!("e2e-{}", Uuid::new_v4());
            let mock = Arc::new(MockConnector::default());

            let mut ext = ExternalIn::new();
            ext.connector.val = Some(Value::make_str(&name));
            ext.address.val = Some(Value::make_str("sensor"));

            let mut add = Add::new();
            let add_uuid = *add.id();
            connect_output(&mut ext.out, add.inputs_mut()[0]).expect("Connected");

            let mut eng = SingleThreadedEngine::new();
            eng.add_connector(&name, mock.clone()).expect("added");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_mock = mock.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    // Wait for the block to open its subscription.
                    let mut tries = 0;
                    while !driver_mock.has_subscription("sensor") {
                        sleep(Duration::from_millis(50)).await;
                        tries += 1;
                        assert!(tries < 100, "block never subscribed");
                    }

                    driver_mock.feed("sensor", Ok(21.into()));

                    // Poll until the value has propagated through the
                    // pin graph into the Add block's output.
                    let mut tries = 0;
                    loop {
                        let _ = engine_sender
                            .send(InspectBlockReq(channel_id, add_uuid))
                            .await;
                        match receiver.recv().await {
                            Some(InspectBlockRes(Ok(data))) => {
                                if data.outputs["out"].val == 21.into() {
                                    break;
                                }
                            }
                            other => panic!("Expected InspectBlockRes(Ok), got {:?}", other),
                        }
                        tries += 1;
                        assert!(tries < 100, "value never reached the Add output");
                        sleep(Duration::from_millis(50)).await;
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.schedule(ext).unwrap();
            eng.schedule(add).unwrap();
            eng.run().await;

            // Propagate any driver-side panic so assertion failures
            // fail the test instead of vanishing with the thread.
            driver.join().unwrap().unwrap();

            unregister_connector(&name);
        }

        /// Regression: a value written to `in` over the engine's
        /// `WriteBlockInputReq` path lands in the input cache directly
        /// (see `BlockMailboxCmd::WriteInput`), and the command's own
        /// arrival cancels the block's in-flight `execute` — so
        /// [`ExternalOut`]'s freshness detection must not depend on a
        /// snapshot taken inside `execute`.
        #[tokio::test(flavor = "current_thread")]
        async fn write_block_input_fires_external_out() {
            let name = format!("e2e-out-{}", Uuid::new_v4());
            let mock = Arc::new(MockConnector::default());

            let ext = ExternalOut::new();
            let ext_uuid = *ext.id();

            let mut eng = SingleThreadedEngine::new();
            eng.add_connector(&name, mock.clone()).expect("added");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_mock = mock.clone();
            let driver_name = name.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    for (pin, value) in [
                        ("connector", Value::make_str(&driver_name)),
                        ("address", Value::make_str("topic")),
                        ("in", 42.into()),
                    ] {
                        let _ = engine_sender
                            .send(WriteBlockInputReq(
                                channel_id,
                                ext_uuid,
                                pin.to_string(),
                                value,
                            ))
                            .await;
                        match receiver.recv().await {
                            Some(WriteBlockInputRes(Ok(_))) => {}
                            other => panic!("Expected WriteBlockInputRes(Ok), got {:?}", other),
                        }
                    }

                    // Poll for the publish. Give up after ~2s and shut
                    // the engine down either way — the main-thread
                    // assertion reports a lost value; panicking here
                    // before `Shutdown` would hang `eng.run()` instead
                    // of failing the test.
                    for _ in 0..40 {
                        if !driver_mock.published.lock().unwrap().is_empty() {
                            break;
                        }
                        sleep(Duration::from_millis(50)).await;
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.schedule(ext).unwrap();
            eng.run().await;
            driver.join().unwrap().unwrap();

            assert_eq!(
                mock.published.lock().unwrap().clone(),
                vec![("topic".to_string(), 42.into())],
                "an `in` value delivered over WriteBlockInputReq is published"
            );

            unregister_connector(&name);
        }

        /// Same regression as `write_block_input_fires_external_out`,
        /// for the [`Request`] block's identical freshness detection.
        #[tokio::test(flavor = "current_thread")]
        async fn write_block_input_fires_request() {
            let name = format!("e2e-req-{}", Uuid::new_v4());
            let mock = Arc::new(MockConnector::default());

            let req = Request::new();
            let req_uuid = *req.id();

            let mut eng = SingleThreadedEngine::new();
            eng.add_connector(&name, mock.clone()).expect("added");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_mock = mock.clone();
            let driver_name = name.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    for (pin, value) in [
                        ("connector", Value::make_str(&driver_name)),
                        ("address", Value::make_str("route")),
                        ("in", 42.into()),
                    ] {
                        let _ = engine_sender
                            .send(WriteBlockInputReq(
                                channel_id,
                                req_uuid,
                                pin.to_string(),
                                value,
                            ))
                            .await;
                        match receiver.recv().await {
                            Some(WriteBlockInputRes(Ok(_))) => {}
                            other => panic!("Expected WriteBlockInputRes(Ok), got {:?}", other),
                        }
                    }

                    // See `write_block_input_fires_external_out` for
                    // why this gives up quietly instead of panicking.
                    for _ in 0..40 {
                        if !driver_mock.requests.lock().unwrap().is_empty() {
                            break;
                        }
                        sleep(Duration::from_millis(50)).await;
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.schedule(req).unwrap();
            eng.run().await;
            driver.join().unwrap().unwrap();

            assert_eq!(
                mock.requests.lock().unwrap().clone(),
                vec![("route".to_string(), 42.into())],
                "an `in` value delivered over WriteBlockInputReq is sent"
            );

            unregister_connector(&name);
        }

        /// Regression: `load_program` pushes saved pin constants through
        /// the same `WriteInput` mailbox path, so a program saved with an
        /// initial `in` value on an [`ExternalOut`] must publish that
        /// value after load.
        #[tokio::test(flavor = "current_thread")]
        async fn load_program_fires_saved_external_out_input() {
            let name = format!("e2e-load-out-{}", Uuid::new_v4());
            let mock = Arc::new(MockConnector::default());

            let ext_uuid = Uuid::new_v4();
            let mut inputs = std::collections::BTreeMap::new();
            inputs.insert(
                "in".to_string(),
                PinValue {
                    value: 42.into(),
                    is_connected: false,
                },
            );
            inputs.insert(
                "connector".to_string(),
                PinValue {
                    value: Value::make_str(&name),
                    is_connected: false,
                },
            );
            inputs.insert(
                "address".to_string(),
                PinValue {
                    value: Value::make_str("topic"),
                    is_connected: false,
                },
            );

            let mut blocks = std::collections::BTreeMap::new();
            blocks.insert(
                ext_uuid.to_string(),
                ProgramBlock {
                    name: "ExternalOut".to_string(),
                    lib: "core".to_string(),
                    label: None,
                    positions: None,
                    inputs,
                    outputs: Default::default(),
                },
            );

            let program = Program {
                name: Some("saved-external-out".to_string()),
                description: None,
                blocks,
                links: Default::default(),
            };

            let mut eng = SingleThreadedEngine::new();
            eng.add_connector(&name, mock.clone()).expect("added");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_mock = mock.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    let _ = engine_sender
                        .send(LoadProgramReq(channel_id, program.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(LoadProgramRes(Ok(()))) => {}
                        other => panic!("Expected LoadProgramRes(Ok), got {:?}", other),
                    }

                    // See `write_block_input_fires_external_out` for
                    // why this gives up quietly instead of panicking.
                    for _ in 0..40 {
                        if !driver_mock.published.lock().unwrap().is_empty() {
                            break;
                        }
                        sleep(Duration::from_millis(50)).await;
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();

            assert_eq!(
                mock.published.lock().unwrap().clone(),
                vec![("topic".to_string(), 42.into())],
                "a saved initial `in` value is published after program load"
            );

            unregister_connector(&name);
        }
    }

    mod connector_lifecycle {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };

        use super::*;
        use crate::base::connector::{get_connector, register_connector, unregister_connector};
        use crate::base::engine::messages::EngineMessage::{
            AddConnectorReq, AddConnectorRes, ListConnectorsReq, ListConnectorsRes,
            RemoveConnectorReq, RemoveConnectorRes, Reset,
        };
        use crate::tokio_impl::engine::connectors::test_support::{
            FailingStartConnector, FlagConnector,
        };

        /// The connector registry is process-global and tests run in
        /// parallel, so every test uses a unique name.
        fn setup(
            prefix: &str,
        ) -> (
            String,
            Arc<AtomicBool>,
            Arc<AtomicBool>,
            SingleThreadedEngine,
        ) {
            let name = format!("{prefix}-{}", Uuid::new_v4());
            let started = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));

            let mut eng = SingleThreadedEngine::new();
            eng.add_connector(
                &name,
                Arc::new(FlagConnector {
                    started: started.clone(),
                    stopped: stopped.clone(),
                }),
            )
            .expect("connector added");

            (name, started, stopped, eng)
        }

        #[tokio::test(flavor = "current_thread")]
        async fn duplicate_name_is_rejected() {
            let (name, _, _, mut eng) = setup("dup");
            let dup = Arc::new(FlagConnector {
                started: Arc::default(),
                stopped: Arc::default(),
            });
            eng.add_connector(&name, dup)
                .expect_err("duplicate connector name is rejected");
            unregister_connector(&name);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn started_on_run_and_stopped_on_shutdown() {
            let (name, started, stopped, mut eng) = setup("shutdown");

            let (sender, _receiver) = mpsc::channel(32);
            let engine_sender = eng.create_message_channel(Uuid::new_v4(), sender);

            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    sleep(Duration::from_millis(100)).await;
                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();

            assert!(started.load(Ordering::SeqCst), "start driven by run()");
            assert!(stopped.load(Ordering::SeqCst), "stop driven on shutdown");
            assert!(
                get_connector(&name).is_some(),
                "shutdown keeps the connector registered for a re-run"
            );
            unregister_connector(&name);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn reset_stops_and_unregisters() {
            let (name, started, stopped, mut eng) = setup("reset");

            let (sender, _receiver) = mpsc::channel(32);
            let engine_sender = eng.create_message_channel(Uuid::new_v4(), sender);

            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    sleep(Duration::from_millis(100)).await;
                    let _ = engine_sender.send(Reset).await;
                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();

            assert!(started.load(Ordering::SeqCst), "start driven by run()");
            assert!(stopped.load(Ordering::SeqCst), "stop driven on reset");
            assert!(
                get_connector(&name).is_none(),
                "reset unregisters the connector"
            );
        }

        /// A connector registered globally while the engine is running
        /// can be attached, listed, and detached over the message
        /// channel.
        #[tokio::test(flavor = "current_thread")]
        async fn dynamic_add_list_remove_over_messages() {
            let name = format!("dynamic-{}", Uuid::new_v4());
            let started = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));

            // Registered globally, NOT engine-managed yet — attaching
            // over the message channel is what puts it under
            // engine management.
            register_connector(
                &name,
                Arc::new(FlagConnector {
                    started: started.clone(),
                    stopped: stopped.clone(),
                }),
            )
            .expect("registered");

            let mut eng = SingleThreadedEngine::new();
            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver_started = started.clone();
            let driver_stopped = stopped.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    sleep(Duration::from_millis(100)).await;

                    let _ = engine_sender
                        .send(AddConnectorReq(channel_id, driver_name.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(AddConnectorRes(Ok(added))) => assert_eq!(added, driver_name),
                        other => panic!("Expected AddConnectorRes(Ok), got {:?}", other),
                    }
                    assert!(
                        driver_started.load(Ordering::SeqCst),
                        "attach awaits the connector's start"
                    );

                    let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                    match receiver.recv().await {
                        Some(ListConnectorsRes(Ok(names))) => {
                            assert!(names.contains(&driver_name), "attached name is listed")
                        }
                        other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                    }

                    let _ = engine_sender
                        .send(RemoveConnectorReq(channel_id, driver_name.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(RemoveConnectorRes(Ok(removed))) => assert_eq!(removed, driver_name),
                        other => panic!("Expected RemoveConnectorRes(Ok), got {:?}", other),
                    }
                    assert!(
                        driver_stopped.load(Ordering::SeqCst),
                        "detach awaits the connector's stop"
                    );
                    assert!(
                        get_connector(&driver_name).is_none(),
                        "detach unregisters the connector"
                    );

                    let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                    match receiver.recv().await {
                        Some(ListConnectorsRes(Ok(names))) => {
                            assert!(!names.contains(&driver_name), "detached name is not listed")
                        }
                        other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();
        }

        /// A failed attach must not leak a partially-started connector:
        /// `start` may have spawned IO tasks before erroring, so the
        /// engine stops the handle before reporting the error. The
        /// name stays registered so a retry remains possible.
        #[tokio::test(flavor = "current_thread")]
        async fn failed_attach_stops_the_connector() {
            let name = format!("failing-start-{}", Uuid::new_v4());
            let stopped = Arc::new(AtomicBool::new(false));
            register_connector(
                &name,
                Arc::new(FailingStartConnector {
                    stopped: stopped.clone(),
                }),
            )
            .expect("registered");

            let mut eng = SingleThreadedEngine::new();
            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver_stopped = stopped.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    sleep(Duration::from_millis(100)).await;

                    let _ = engine_sender
                        .send(AddConnectorReq(channel_id, driver_name.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(AddConnectorRes(Err(_))) => {}
                        other => panic!(
                            "Expected AddConnectorRes(Err) on start failure, got {:?}",
                            other
                        ),
                    }
                    assert!(
                        driver_stopped.load(Ordering::SeqCst),
                        "failed attach stops the connector before replying"
                    );
                    assert!(
                        get_connector(&driver_name).is_some(),
                        "failed attach keeps the connector registered for a retry"
                    );

                    let _ = engine_sender.send(ListConnectorsReq(channel_id)).await;
                    match receiver.recv().await {
                        Some(ListConnectorsRes(Ok(names))) => {
                            assert!(
                                !names.contains(&driver_name),
                                "failed attach is not tracked"
                            )
                        }
                        other => panic!("Expected ListConnectorsRes(Ok), got {:?}", other),
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();

            unregister_connector(&name);
        }

        /// Attach/detach error cases: an unregistered name, a name the
        /// engine already manages, and detaching an unmanaged name.
        #[tokio::test(flavor = "current_thread")]
        async fn dynamic_add_remove_error_cases() {
            let (name, _, _, mut eng) = setup("dynamic-errors");

            let (sender, mut receiver) = mpsc::channel(32);
            let channel_id = Uuid::new_v4();
            let engine_sender = eng.create_message_channel(channel_id, sender);

            let driver_name = name.clone();
            let driver = thread::spawn(move || {
                let rt = Runtime::new().expect("RT");
                let handle = rt.spawn(async move {
                    sleep(Duration::from_millis(100)).await;

                    let missing = format!("missing-{}", Uuid::new_v4());
                    let _ = engine_sender
                        .send(AddConnectorReq(channel_id, missing.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(AddConnectorRes(Err(_))) => {}
                        other => panic!(
                            "Expected AddConnectorRes(Err) for an unregistered name, got {:?}",
                            other
                        ),
                    }

                    let _ = engine_sender
                        .send(AddConnectorReq(channel_id, driver_name.clone()))
                        .await;
                    match receiver.recv().await {
                        Some(AddConnectorRes(Err(_))) => {}
                        other => panic!(
                            "Expected AddConnectorRes(Err) for an already-managed name, got {:?}",
                            other
                        ),
                    }

                    let _ = engine_sender
                        .send(RemoveConnectorReq(channel_id, missing))
                        .await;
                    match receiver.recv().await {
                        Some(RemoveConnectorRes(Err(_))) => {}
                        other => panic!(
                            "Expected RemoveConnectorRes(Err) for an unmanaged name, got {:?}",
                            other
                        ),
                    }

                    let _ = engine_sender.send(Shutdown).await;
                });
                rt.block_on(handle)
            });

            eng.run().await;
            driver.join().unwrap().unwrap();

            unregister_connector(&name);
        }
    }
}
