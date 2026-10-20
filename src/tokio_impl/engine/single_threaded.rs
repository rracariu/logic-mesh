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
//! The actor tasks live on the engine's own `LocalSet`, so dropping the
//! engine — idle, or along with an unfinished `run()` future — drops
//! every actor task with it.
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
//! - `engine` — [`SingleThreadedEngine`] itself: lifecycle, the [`Engine`]
//!   trait impl, sync configuration helpers, and the async APIs that
//!   `message_dispatch` routes external requests to.
//! - `graph` (shared with the multi-threaded engine) — the
//!   [`BlockHandle`]s and the running-phase operations over them: pin
//!   writes, link wiring, block removal, program save and load.
//!
//! [`Engine`]: crate::base::engine::Engine
//! [`BlockHandle`]: crate::base::engine::BlockHandle

mod actor;
mod engine;

pub use engine::SingleThreadedEngine;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::base;
    use crate::base::program::{
        Program, ProgramBlock,
        data::{LinkData, PinValue},
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
            .validate_link(&link(missing.to_string(), add_uuid.to_string(), "in0"))
            .expect_err("unknown source block is rejected");
        assert_matches!(
            err,
            Error::Engine(EngineError::BlockInstanceNotFound { id }) if id == missing
        );

        let err = eng
            .validate_link(&link(
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
        use crate::base::block::connect_output;

        let mut add1 = Add::new();
        let add_uuid = *add1.id();

        let mut sine1 = SineWave::new();

        sine1.amplitude.set(3);
        sine1.freq.set(200);
        connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

        let mut sine2 = SineWave::new();
        sine2.amplitude.set(7);
        sine2.freq.set(400);

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

    /// The engine and its block handles summarise themselves without
    /// dumping blocks, descriptors or channels.
    #[tokio::test(flavor = "current_thread")]
    async fn debug_summarises_the_engine_and_its_blocks() {
        let mut source = Add::new();
        let mut target = Add::new();
        let target_id = *target.id();
        source
            .connect_output("out", target.inputs_mut()[0])
            .expect("not linked yet");

        let mut eng = SingleThreadedEngine::new();
        eng.schedule(source).expect("scheduled");
        eng.schedule(target).expect("scheduled");

        assert_eq!(
            format!("{eng:?}"),
            "SingleThreadedEngine { phase: \"Idle\", executing: false, blocks: 2, links: 1, \
             pending_programs: 0, connectors: [], scan_interval: 200ms, .. }"
        );
        assert_eq!(
            format!("{:?}", eng.block_handle(&target_id).expect("scheduled")),
            format!(
                "BlockHandle {{ id: {target_id}, name: \"Add\", library: \"core\", \
                 label: None, position: None, .. }}"
            )
        );
    }

    /// Regression: a save always returned no name and no description,
    /// and lost the ids of the links it had loaded — see
    /// `test_support::round_trip`.
    mod round_trip {
        use super::*;
        use crate::tokio_impl::engine::test_support::round_trip;

        #[tokio::test(flavor = "current_thread")]
        async fn a_saved_program_reloads_unchanged() {
            round_trip::a_saved_program_reloads_unchanged(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn reset_forgets_the_program_identity() {
            round_trip::reset_forgets_the_program_identity(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_scheduled_while_idle_saves_its_identity() {
            round_trip::a_program_scheduled_while_idle_saves_its_identity(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_1_0_program_loads_and_saves_in_the_current_format() {
            round_trip::a_1_0_program_loads_and_saves_in_the_current_format(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_block_saved_at_a_compatible_version_loads() {
            round_trip::a_block_saved_at_a_compatible_version_loads(SingleThreadedEngine::new())
                .await;
        }
    }

    /// Regression: a load that failed on a block type, a link or a pin
    /// left the blocks scheduled before the failure in place, half
    /// wired, and ignored unknown pin names in saved values — see
    /// `test_support::atomic_loads`.
    mod atomic_loads {
        use super::*;
        use crate::tokio_impl::engine::test_support::atomic_loads;

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_failing_its_checks_loads_nothing() {
            atomic_loads::a_program_failing_its_checks_loads_nothing(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_failing_its_checks_schedules_nothing() {
            atomic_loads::a_program_failing_its_checks_schedules_nothing(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_failing_to_wire_is_removed_again() {
            atomic_loads::a_program_failing_to_wire_is_removed_again(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_scheduled_while_idle_drops_a_link_it_fails_to_wire() {
            atomic_loads::a_program_scheduled_while_idle_drops_a_link_it_fails_to_wire(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_scheduled_while_idle_gets_its_pin_values() {
            atomic_loads::a_program_scheduled_while_idle_gets_its_pin_values(
                SingleThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: once a block's actor was gone, the block could not
    /// be removed, saving the program failed, and its links could not be
    /// removed — see `test_support::dead_actors`.
    mod dead_actors {
        use super::*;
        use crate::tokio_impl::engine::test_support::dead_actors;

        #[tokio::test(flavor = "current_thread")]
        async fn a_block_whose_actor_is_gone_is_saved_without_pins() {
            dead_actors::a_block_whose_actor_is_gone_is_saved_without_pins(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_link_out_of_a_gone_actor_is_removed() {
            dead_actors::a_link_out_of_a_gone_actor_is_removed(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_block_whose_actor_is_gone_is_removed() {
            dead_actors::a_block_whose_actor_is_gone_is_removed(SingleThreadedEngine::new()).await;
        }
    }

    /// Regression: a block had to implement `BlockStaticDesc` to be a
    /// `Block` at all, so a block described at runtime — and the
    /// registry's type-erased block — stubbed it with `unimplemented!()`
    /// — see `test_support::runtime_descs`.
    mod runtime_descs {
        use super::*;
        use crate::tokio_impl::engine::test_support::runtime_descs;

        #[tokio::test(flavor = "current_thread")]
        async fn a_block_with_a_runtime_desc_runs_and_saves() {
            runtime_descs::a_block_with_a_runtime_desc_runs_and_saves(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_registered_block_loads_and_saves() {
            runtime_descs::a_registered_block_loads_and_saves(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_registered_block_is_found_by_its_name_alone() {
            runtime_descs::a_registered_block_is_found_by_its_name_alone(
                SingleThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: a panic in `execute` killed the block's actor task —
    /// the block never showed Fault, downstream never faulted, and the
    /// block could no longer be saved or removed.
    mod panics {
        use super::*;
        use crate::tokio_impl::engine::test_support::panics;

        #[tokio::test(flavor = "current_thread")]
        async fn panic_faults_the_block_and_downstream() {
            panics::panic_faults_the_block_and_downstream(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn panicked_block_recovers_after_a_clean_cycle() {
            panics::panicked_block_recovers_after_a_clean_cycle(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_block_panicking_every_cycle_backs_off() {
            panics::a_block_panicking_every_cycle_backs_off(SingleThreadedEngine::new()).await;
        }
    }

    /// Dropping the engine drops its `LocalSet`, and every actor task
    /// with it — see `test_support::drops`.
    mod drops {
        use super::*;
        use crate::tokio_impl::engine::test_support::drops;

        #[tokio::test(flavor = "current_thread")]
        async fn dropping_an_idle_engine_ends_its_actors() {
            drops::dropping_an_idle_engine_ends_its_actors(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn dropping_a_shut_down_engine_ends_its_actors() {
            drops::dropping_a_shut_down_engine_ends_its_actors(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn dropping_the_run_future_ends_its_actors() {
            drops::dropping_the_run_future_ends_its_actors(SingleThreadedEngine::new()).await;
        }
    }

    /// The lifecycle contract — see `test_support::lifecycle`.
    mod lifecycle {
        use super::*;
        use crate::tokio_impl::engine::test_support::lifecycle;

        /// Guards the contract the single-threaded engine already kept.
        #[tokio::test(flavor = "current_thread")]
        async fn blocks_survive_shutdown() {
            lifecycle::blocks_survive_shutdown(SingleThreadedEngine::new()).await;
        }

        /// Regression: dispatching a request drove the `LocalSet`, so
        /// blocks kept executing while paused.
        #[tokio::test(flavor = "current_thread")]
        async fn pause_holds_blocks_and_answers_requests() {
            lifecycle::pause_holds_blocks_and_answers_requests(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn reset_while_paused() {
            lifecycle::reset_while_paused(SingleThreadedEngine::new()).await;
        }
    }

    /// Regression: a pin write landed in the input's cache only, and an
    /// actor wakes on linked inputs alone, so a write to an unlinked
    /// input never ran the block — see `test_support::pin_writes`.
    mod pin_writes {
        use super::*;
        use crate::tokio_impl::engine::test_support::pin_writes;

        #[tokio::test(flavor = "current_thread")]
        async fn writing_an_unlinked_block_recomputes_it() {
            pin_writes::writing_an_unlinked_block_recomputes_it(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn writing_a_constant_input_recomputes_a_linked_block() {
            pin_writes::writing_a_constant_input_recomputes_a_linked_block(
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_fix_written_to_an_unlinked_input_clears_a_fault() {
            pin_writes::a_fix_written_to_an_unlinked_input_clears_a_fault(
                SingleThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: the scan interval was one process-wide value, which
    /// every engine shared. See `test_support::scan_intervals`.
    mod scan_intervals {
        use super::*;
        use crate::tokio_impl::engine::test_support::scan_intervals;

        #[tokio::test(flavor = "current_thread")]
        async fn each_engine_keeps_its_own_scan_interval() {
            scan_intervals::each_engine_keeps_its_own_scan_interval(
                SingleThreadedEngine::new(),
                SingleThreadedEngine::new(),
            )
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_new_scan_interval_reaches_scheduled_blocks() {
            scan_intervals::a_new_scan_interval_reaches_scheduled_blocks(
                SingleThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: generic code could not configure an engine through
    /// the `Engine` trait. See `test_support::configuration`.
    mod configuration {
        use super::*;
        use crate::tokio_impl::engine::test_support::configuration;

        #[tokio::test(flavor = "current_thread")]
        async fn an_engine_is_configured_through_the_trait() {
            configuration::an_engine_is_configured_through_the_trait(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn editor_metadata_is_set_through_the_trait() {
            configuration::editor_metadata_is_set_through_the_trait(SingleThreadedEngine::new())
                .await;
        }
    }

    /// Regression: a block that rejected a request answered with a bare
    /// message, which the engine wrapped in a `BlockRequestRejected`
    /// string naming neither the block nor the pin. See
    /// `test_support::rejections`.
    mod rejections {
        use super::*;
        use crate::tokio_impl::engine::test_support::rejections;

        #[tokio::test(flavor = "current_thread")]
        async fn writing_a_missing_pin_names_it() {
            rejections::writing_a_missing_pin_names_it(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn linking_the_same_pins_twice_is_a_connect_error() {
            rejections::linking_the_same_pins_twice_is_a_connect_error(SingleThreadedEngine::new())
                .await;
        }
    }

    /// Regression: wiring a link always made up a fresh id, so a link
    /// lost the id a connect request named or a loaded program saved it
    /// under — and removing it by that id failed. See
    /// `test_support::link_ids`.
    mod link_ids {
        use super::*;
        use crate::tokio_impl::engine::test_support::link_ids;

        #[tokio::test(flavor = "current_thread")]
        async fn connecting_keeps_a_named_link_id() {
            link_ids::connecting_keeps_a_named_link_id(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn connecting_rejects_a_taken_or_invalid_link_id() {
            link_ids::connecting_rejects_a_taken_or_invalid_link_id(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_loaded_link_keeps_its_saved_id() {
            link_ids::a_loaded_link_keeps_its_saved_id(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_program_reusing_a_link_id_is_rejected() {
            link_ids::a_program_reusing_a_link_id_is_rejected(SingleThreadedEngine::new()).await;
        }
    }

    /// Regression: every request addressed to a block cancels its
    /// cycle, and the re-issued operation restarted its timeout, so
    /// steady requests kept a slow publish or request from ever timing
    /// out; a subscribe had no timeout at all — see
    /// `test_support::connector_deadlines`.
    mod connector_deadlines {
        use super::*;
        use crate::tokio_impl::engine::test_support::connector_deadlines;

        #[tokio::test(flavor = "current_thread")]
        async fn external_out_times_out_under_requests() {
            connector_deadlines::external_out_times_out_under_requests(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn request_times_out_under_requests() {
            connector_deadlines::request_times_out_under_requests(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn external_in_times_out_under_requests() {
            connector_deadlines::external_in_times_out_under_requests(SingleThreadedEngine::new())
                .await;
        }

        /// Regression: an operation's deadline was fixed from the
        /// `timeout` pin when the operation was first issued, so a
        /// timeout written while it was in flight — as a program load
        /// writes it, after `in` — was ignored until the next value.
        #[tokio::test(flavor = "current_thread")]
        async fn external_out_applies_a_late_timeout() {
            connector_deadlines::external_out_applies_a_late_timeout(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn request_applies_a_late_timeout() {
            connector_deadlines::request_applies_a_late_timeout(SingleThreadedEngine::new()).await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn external_in_applies_a_late_timeout() {
            connector_deadlines::external_in_applies_a_late_timeout(SingleThreadedEngine::new())
                .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn a_loaded_timeout_applies_to_the_first_operation() {
            connector_deadlines::a_loaded_timeout_applies_to_the_first_operation(
                SingleThreadedEngine::new(),
            )
            .await;
        }
    }

    /// Regression: a reply variant sent to the engine hit an
    /// `unreachable!` and panicked the event loop. It is now ignored,
    /// and the engine keeps answering.
    #[tokio::test(flavor = "current_thread")]
    async fn stray_reply_is_ignored() {
        use crate::base::engine::messages::EngineMessage::{
            AddBlockRes, ListConnectorsReq, ListConnectorsRes,
        };
        use crate::tokio_impl::engine::test_support::drive;

        drive(SingleThreadedEngine::new(), |mut client| async move {
            client.send(AddBlockRes(Ok(Uuid::new_v4()))).await;

            let reply = client.request(ListConnectorsReq(client.id)).await;
            assert!(
                matches!(reply, ListConnectorsRes(Ok(_))),
                "Expected ListConnectorsRes(Ok), got {:?}",
                reply
            );
        })
        .await;
    }

    /// Regression: scheduling an id that is already taken used to
    /// replace the scheduled block's handle, orphaning its actor.
    mod duplicate_ids {
        use assert_matches::assert_matches;

        use super::*;
        use crate::base::engine::messages::EngineMessage::{AddBlockReq, AddBlockRes};
        use crate::base::error::{EngineError, Error};
        use crate::tokio_impl::engine::test_support::{Client, add_program, drive};

        #[tokio::test(flavor = "current_thread")]
        async fn schedule_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("first schedule");

            let err = eng
                .schedule(Add::new_uuid(id))
                .expect_err("second schedule is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert_eq!(eng.block_handles().len(), 1);
        }

        #[tokio::test(flavor = "current_thread")]
        async fn schedule_local_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();
            eng.schedule_local(Add::new_uuid(id))
                .expect("first schedule");

            let err = eng
                .schedule_local(Add::new_uuid(id))
                .expect_err("second schedule is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert_eq!(eng.block_handles().len(), 1);
        }

        /// The whole program is checked up front: the program's other,
        /// fresh block is not scheduled either.
        #[tokio::test(flavor = "current_thread")]
        async fn schedule_program_blocks_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let fresh = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            let program = add_program([id.to_string().as_str(), fresh.to_string().as_str()]);
            let err = eng
                .schedule_program_blocks(&program)
                .expect_err("program reusing a scheduled id is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert!(eng.block_handle(&fresh).is_none(), "nothing was scheduled");
            assert_eq!(eng.block_handles().len(), 1);
        }

        /// Two program keys can spell the same UUID differently.
        #[tokio::test(flavor = "current_thread")]
        async fn schedule_program_blocks_rejects_an_id_named_twice() {
            let id = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();

            let lower = id.to_string();
            let upper = lower.to_uppercase();
            let err = eng
                .schedule_program_blocks(&add_program([lower.as_str(), upper.as_str()]))
                .expect_err("program naming an id twice is rejected");
            assert_matches!(
                err,
                Error::Engine(EngineError::DuplicateBlockId { id: dup }) if dup == id
            );
            assert!(eng.block_handles().is_empty(), "nothing was scheduled");
        }

        #[tokio::test(flavor = "current_thread")]
        async fn add_block_req_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            drive(eng, move |mut client| async move {
                let reply = client
                    .request(AddBlockReq(
                        client.id,
                        "Add".to_string(),
                        Some(id.to_string()),
                        None,
                    ))
                    .await;
                assert_matches!(
                    reply,
                    AddBlockRes(Err(msg))
                        if msg == EngineError::DuplicateBlockId { id }.to_string()
                );

                // The scheduled block is untouched and still answers.
                let reply = client.request(InspectBlockReq(client.id, id)).await;
                assert_matches!(reply, InspectBlockRes(Ok(def)) if def.id == id.to_string());
                assert_eq!(scheduled_ids(&mut client).await, vec![id.to_string()]);
            })
            .await;
        }

        #[tokio::test(flavor = "current_thread")]
        async fn load_program_req_rejects_a_scheduled_id() {
            let id = Uuid::new_v4();
            let fresh = Uuid::new_v4();
            let mut eng = SingleThreadedEngine::new();
            eng.schedule(Add::new_uuid(id)).expect("scheduled");

            let program = add_program([id.to_string().as_str(), fresh.to_string().as_str()]);
            drive(eng, move |mut client| async move {
                let reply = client.request(LoadProgramReq(client.id, program)).await;
                assert_matches!(
                    reply,
                    LoadProgramRes(Err(msg))
                        if msg == EngineError::DuplicateBlockId { id }.to_string()
                );
                assert_eq!(
                    scheduled_ids(&mut client).await,
                    vec![id.to_string()],
                    "nothing was scheduled"
                );
            })
            .await;
        }

        /// The ids of the running engine's blocks, from its saved program.
        async fn scheduled_ids(client: &mut Client) -> Vec<String> {
            match client.request(GetCurrentProgramReq(client.id)).await {
                GetCurrentProgramRes(Ok(program)) => program.blocks.into_keys().collect(),
                other => panic!("Expected GetCurrentProgramRes(Ok), got {:?}", other),
            }
        }
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
            use crate::base::block::connect_output;

            let name = format!("e2e-{}", Uuid::new_v4());
            let mock = Arc::new(MockConnector::default());

            let mut ext = ExternalIn::new();
            ext.connector.set(Value::make_str(&name));
            ext.address.set(Value::make_str("sensor"));

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
            let mut block = ProgramBlock::new("ExternalOut", "core");
            block.inputs = inputs;
            blocks.insert(ext_uuid.to_string(), block);

            let mut program = Program::new();
            program.name = Some("saved-external-out".to_string());
            program.blocks = blocks;

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
            FailingStartConnector, FlagConnector, start_failure_reply,
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

            let eng = eng.run().await;
            driver.join().unwrap().unwrap();

            assert!(started.load(Ordering::SeqCst), "start driven by run()");
            assert!(stopped.load(Ordering::SeqCst), "stop driven on shutdown");
            assert!(
                get_connector(&name).is_some(),
                "shutdown keeps the connector registered for a re-run"
            );
            drop(eng);
            assert!(
                get_connector(&name).is_none(),
                "dropping the engine unregisters the connector"
            );
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
                    let reply = match receiver.recv().await {
                        Some(AddConnectorRes(Err(reply))) => reply,
                        other => panic!(
                            "Expected AddConnectorRes(Err) on start failure, got {:?}",
                            other
                        ),
                    };
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
                    reply
                });
                rt.block_on(handle)
            });

            eng.run().await;
            let reply = driver.join().unwrap().unwrap();
            unregister_connector(&name);

            // The reply carries the start error's source too
            // (regression: it printed only the error's own message).
            assert_eq!(reply, start_failure_reply(&name));
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
