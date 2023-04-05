// Copyright (c) 2022-2023, IntriSemantics Corp.

#![allow(incomplete_features)]
#![feature(async_closure)]
#![feature(async_fn_in_trait)]
#![feature(trait_alias)]

#[macro_use]
extern crate block_macro;

pub mod base;
pub mod blocks;
mod tokio_impl;
pub use tokio_impl::engine::*;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::base;
    use crate::blocks::{maths::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};

    use crate::tokio_impl::engine::Engine;
    use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
    use uuid::Uuid;

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test(flavor = "current_thread")]
    async fn engine_test() {
        let mut add1 = Add::new("block1");

        let mut sine1 = SineWave::new("a");

        sine1.amplitude.val = Some(3.into());
        sine1.freq.val = Some(200.into());
        sine1.connect(add1.inputs_mut()[0]);

        let mut sine2 = SineWave::new("b");
        sine2.amplitude.val = Some(7.into());
        sine2.freq.val = Some(400.into());

        sine2.connect(add1.inputs_mut()[1]);

        let mut eng = Engine::new();

        let (sender, mut receiver) = mpsc::channel(32);
        let engine_sender = eng.message_handles(Uuid::new_v4(), sender.clone());

        thread::spawn(move || {
            let rt = Runtime::new().expect("RT");

            let handle = rt.spawn(async move {
                loop {
                    sleep(Duration::from_millis(500)).await;
                    let _ = engine_sender
                        .send(base::engine_messages::EngineMessage::Shutdown)
                        .await;

                    let _ = receiver.try_recv();
                }
            });

            rt.block_on(async { handle.await })
        });

        eng.schedule(add1);
        eng.schedule(sine1);
        eng.schedule(sine2);

        eng.run().await;
    }
}
