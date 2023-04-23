// Copyright (c) 2022-2023, IntriSemantics Corp.

use crate::base::block::{Block, BlockDesc, BlockDescAccess, BlockProps};
use crate::base::input::InputProps;
use crate::blocks::maths::Add;
use crate::blocks::misc::{Random, SineWave};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::InputImpl;

type DynBlock =
    dyn BlockProps<Rx = <InputImpl as InputProps>::Rx, Tx = <InputImpl as InputProps>::Tx>;
type MapType = BTreeMap<String, BlockData>;
type BlockRegistry = Mutex<MapType>;

pub struct BlockData {
    pub desc: &'static BlockDesc,
    pub make: fn() -> Box<DynBlock>,
}

lazy_static! {
    /// The block registry
    /// This is a static variable that is initialized once and then
    /// used throughout the lifetime of the program.
    pub static ref  BLOCKS: BlockRegistry = {
        let mut reg = BTreeMap::new();
        register_impl::<Add>(&mut reg);
        register_impl::<Random>(&mut reg);
        register_impl::<SineWave>(&mut reg);

        reg.into()
    };
}
/// Get a block from the registry
/// # Arguments
/// - name: The name of the block to get
/// # Returns
/// A boxed block
pub fn make(name: &str) -> Option<Box<DynBlock>> {
    let reg = BLOCKS.lock().expect("Block registry is locked");

    if let Some(data) = reg.get(name) {
        Some((data.make)())
    } else {
        None
    }
}

/// Register a block with the registry
/// # Arguments
/// - B: The block type to register
/// # Panics
/// Panics if the block registry is already locked
pub fn register<
    B: Block<Rx = <InputImpl as InputProps>::Rx, Tx = <InputImpl as InputProps>::Tx>
        + Default
        + 'static,
>() {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    register_impl::<B>(&mut reg);
}

fn register_impl<
    B: Block<Rx = <InputImpl as InputProps>::Rx, Tx = <InputImpl as InputProps>::Tx>
        + Default
        + 'static,
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
