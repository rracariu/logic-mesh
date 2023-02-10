// Copyright (c) 2022-2023, IntriSemantics Corp.

#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(trait_alias)]

#[macro_use]
extern crate block_macro;

use crate::base::block::Block;
use base::block::{BlockConnect, BlockProps};
use blocks::{maths::Add, misc::SineWave};
use futures::{future::select_all, FutureExt};

pub mod base;
pub mod blocks;
mod tokio_impl;

#[tokio::main]
async fn main() {
    let mut block1 = Add::new("block1");

    let mut sine = SineWave::new("");
    sine.amplitude.val = Some(3.into());
    sine.freq.val = Some(100.into());

    sine.connect(block1.inputs_mut()[0]);

    loop {
        let blocks = [block1.execute().boxed(), sine.execute().boxed()];
        select_all(blocks).await;
    }
}
