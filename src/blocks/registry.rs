// Copyright (c) 2022-2023, IntriSemantics Corp.

use crate::base::block::{Block, BlockDesc, BlockDescAccess, BlockProps};
use crate::blocks::maths::Add;
use crate::blocks::misc::{Random, SineWave};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::Mutex;

type DynBlock = dyn BlockProps<Rx = <Add as BlockProps>::Rx, Tx = <Add as BlockProps>::Tx>;
type MapType = BTreeMap<String, BlockData>;
type BlockRegistry = Mutex<MapType>;

pub struct BlockData {
    pub desc: &'static BlockDesc,
    pub make: fn() -> Box<DynBlock>,
}

lazy_static! {
    /// The list of all registered blocks
    pub static ref  BLOCKS: BlockRegistry = {
        let mut reg = BTreeMap::new();
        register_impl::<Add>(&mut reg);
        register_impl::<Random>(&mut reg);
        register_impl::<SineWave>(&mut reg);

        reg.into()
    };
}

fn make(name: &str) -> Box<DynBlock> {
    if let Ok(reg) = BLOCKS.lock() {
        if let Some(data) = reg.get(name) {
            (data.make)()
        } else {
            panic!("Failed to lock block registry");
        }
    } else {
        panic!("Failed to lock block registry");
    }
}

pub fn register<
    B: Block<Rx = <Add as BlockProps>::Rx, Tx = <Add as BlockProps>::Tx> + Default + 'static,
>() {
    let reg = BLOCKS.lock();

    if let Ok(mut reg) = reg {
        register_impl::<B>(&mut reg);
    }
}

fn register_impl<
    B: Block<Rx = <Add as BlockProps>::Rx, Tx = <Add as BlockProps>::Tx> + Default + 'static,
>(
    reg: &mut MapType,
) {
    reg.insert(<B as BlockDescAccess>::desc().name.clone(), {
        let desc = <B as BlockDescAccess>::desc();
        let make = || -> Box<DynBlock> {
            let block = B::default();
            Box::new(block)
        };

        BlockData { desc, make }
    });
}
