// Copyright (c) 2022-2026, Radu Racariu.

//! Single-threaded engine.
//!
//! ## Architecture
//!
//! Each scheduled block is hosted by its own task on a tokio `LocalSet`.
//! The block is **owned by value** by that task — there is no `UnsafeCell`,
//! no `Rc`-shared mutable state, and no aliasing. Every external operation
//! against a block (write input/output, inspect, wire/teardown links) is
//! routed through a per-block `mpsc` mailbox (`BlockMailboxCmd`).
//!
//! The per-block task loop uses `tokio::select!` to interleave the block's
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
//! ## Module layout
//!
//! - [`mailbox`] — the [`BlockMailboxCmd`] enum and the actor-side
//!   `handle_cmd` that turns a command into mutations on the owned block.
//! - [`actor`] — the per-block task loop (`block_actor_task`) and the
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
    use crate::blocks::{math::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes, Shutdown};

    use super::SingleThreadedEngine;
    use base::engine::Engine;
    use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
    use uuid::Uuid;

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

        eng.schedule(add1);
        eng.schedule(sine1);
        eng.schedule(sine2);

        eng.run().await;
    }
}
