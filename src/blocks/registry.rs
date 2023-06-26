// Copyright (c) 2022-2023, Radu Racariu.

use crate::base::block::{Block, BlockDesc, BlockProps, BlockStaticDesc};
use crate::base::input::InputProps;

use crate::base::engine::Engine;
use crate::blocks::math::{Abs, Add, Cos, Sub};
use crate::blocks::misc::{Random, SineWave};

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::math::{Div, Mul, Sin};
use super::InputImpl;

type DynBlockProps = dyn BlockProps<
    Reader = <InputImpl as InputProps>::Reader,
    Writer = <InputImpl as InputProps>::Writer,
>;
type MapType = BTreeMap<String, BlockEntry>;
type BlockRegistry = Mutex<MapType>;

/// Register a block in the registry
pub struct BlockEntry {
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
		pub fn schedule_block<E>(name: &str, eng: &mut E) -> Result<uuid::Uuid>
		where E : Engine<Reader = <InputImpl as InputProps>::Reader, Writer = <InputImpl as InputProps>::Writer> {

			match name {
				$(
					stringify!($x) => {
						let block = <$x>::new();
						let uuid = *block.id();
						eng.schedule(block);
						Ok(uuid)
					}
				)*
				_ => {
					return Err(anyhow!("Block not found"));
				}
			}

		}

		/// Schedule a block by name and UUID.
		/// See [`schedule_block`] for more details.
		pub fn schedule_block_with_uuid<E>(name: &str, uuid: uuid::Uuid, eng: &mut E) -> Result<uuid::Uuid>
		where E : Engine<Reader = <InputImpl as InputProps>::Reader, Writer = <InputImpl as InputProps>::Writer> {

			match name {
				$(
					stringify!($x) => {
						let block = <$x>::new_uuid(uuid);
						eng.schedule(block);
						Ok(uuid)
					}
				)*
				_ => {
					return Err(anyhow!("Block not found"));
				}
			}

		}
    };
}

register_blocks!(Abs, Add, Sub, Mul, Div, Cos, Sin, Random, SineWave);

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
    B: Block<
            Reader = <InputImpl as InputProps>::Reader,
            Writer = <InputImpl as InputProps>::Writer,
        > + Default
        + 'static,
>() {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    register_impl::<B>(&mut reg);
}

fn register_impl<
    B: Block<
            Reader = <InputImpl as InputProps>::Reader,
            Writer = <InputImpl as InputProps>::Writer,
        > + Default
        + 'static,
>(
    reg: &mut MapType,
) {
    reg.insert(<B as BlockStaticDesc>::desc().name.clone(), {
        let desc = <B as BlockStaticDesc>::desc();
        let make = || -> Box<DynBlockProps> {
            let block = B::default();
            Box::new(block)
        };

        BlockEntry { desc, make }
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

        let mut eng = crate::single_threaded::SingleThreadedEngine::new();

        schedule_block("Add", &mut eng).expect("Block");

        assert!(eng.blocks().iter().any(|b| b.desc().name == "Add"));
    }
}
