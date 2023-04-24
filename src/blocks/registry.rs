// Copyright (c) 2022-2023, IntriSemantics Corp.

use crate::base::block::{Block, BlockDesc, BlockDescAccess, BlockProps};
use crate::base::input::InputProps;
use crate::blocks::maths::Add;
use crate::blocks::misc::{Random, SineWave};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::InputImpl;

type DynBlockProps =
    dyn BlockProps<Rx = <InputImpl as InputProps>::Rx, Tx = <InputImpl as InputProps>::Tx>;
type MapType = BTreeMap<String, BlockData>;
type BlockRegistry = Mutex<MapType>;

pub struct BlockData {
    pub desc: &'static BlockDesc,
    pub make: fn() -> Box<DynBlockProps>,
}

/// Macro for statically registering all the blocks that are
/// available in the system.
#[macro_export]
macro_rules! register_blocks{
    ( $( $x:ty ),* ) => {
		lazy_static! {
			/// The block registry
			/// This is a static variable that is initialized once and then
			/// used throughout the lifetime of the program.
			pub static ref  BLOCKS: BlockRegistry = {
				let mut reg = BTreeMap::new();

				$(
					register_impl::<$x>(&mut reg);
				)*

				reg.into()
			};
		}

		/// Schedule a block by name.
		/// If the block name is valid, it will be scheduled on the engine.
		/// The engine will execute the block if the engine is running.
		/// This requires that the block is statically registered.
		///
		/// # Arguments
		/// - name: The name of the block to schedule
		/// - eng: The engine to schedule the block on
		/// # Returns
		/// A result indicating success or failure
		pub fn schedule_block<E>(name: &str, eng: &mut E) -> Result<(), &'static str>
		where E : crate::base::engine::Engine<Rx = <InputImpl as InputProps>::Rx,Tx = <InputImpl as InputProps>::Tx> {

			match name {
				$(
					stringify!($x) => {
						let block = <$x>::new();
						eng.schedule(block);
						Ok(())
					}
				)*
				_ => {
					return Err("Block not found");
				}
			}

		}
    };
}

register_blocks!(Add, Random, SineWave);

/// Construct a block properties from the registry
/// # Arguments
/// - name: The name of the block to get
/// # Returns
/// A boxed block
pub fn make(name: &str) -> Option<Box<DynBlockProps>> {
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
        let make = || -> Box<DynBlockProps> {
            let block = B::default();
            Box::new(block)
        };

        BlockData { desc, make }
    });
}

#[cfg(test)]
mod test {

    use crate::base::block::connect::connect_output;

    use super::*;

    #[test]
    fn test_registry() {
        let mut add = make("Add").expect("Add block not found");
        let mut random = make("Random").expect("Random block not found");
        let sine = make("SineWave").expect("SineWave block not found");

        assert_eq!(add.desc().name, "Add");
        assert_eq!(random.desc().name, "Random");
        assert_eq!(sine.desc().name, "SineWave");

        let mut outs = random.outputs_mut();
        let mut ins = add.inputs_mut();

        let out = outs.first_mut().unwrap();
        let input = ins.first_mut().unwrap();

        connect_output(*out, *input).unwrap();

        let mut eng = crate::single_threaded::LocalSetEngine::new();

        schedule_block("Add", &mut eng).expect("Block");
    }
}
