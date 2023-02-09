// Copyright (c) 2022-2023, IntriSemantics Corp.

#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
#![feature(trait_alias)]

#[macro_use]
extern crate block_macro;

use crate::base::block::Block;
use base::block::{BlockConnect, BlockProps};
use blocks::maths::Add;
use futures::{future::select_all, FutureExt};
use libhaystack::val::Value;
use tokio_impl::sinewave_block;

pub mod base;
pub mod blocks;
mod tokio_impl;

#[tokio::main]
async fn main() {
    let mut block1 = Add::new("block1");

    let mut block2 = Add::new("block2");

    block1.connect(block2.inputs_mut()[0]);
    block1.connect(block2.inputs_mut()[1]);

    block2.connect(block1.inputs_mut()[0]);
    block2.connect(block1.inputs_mut()[1]);

    let sine = sinewave_block::SineWave::new("");
    sine.id();

    block1.out.set(Value::make_int(2)).await;

    loop {
        let blocks = [block1.execute().boxed(), block2.execute().boxed()];
        select_all(blocks).await;
    }
}
