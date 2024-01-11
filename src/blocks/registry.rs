// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::block::{BlockDesc, BlockProps, BlockStaticDesc};
use crate::base::input::input_reader::InputReader;
use crate::base::input::InputProps;
use libhaystack::val::Value;

use crate::base::engine::Engine;
use crate::blocks::bitwise::{BitwiseAnd, BitwiseNot, BitwiseOr, BitwiseXor};
use crate::blocks::collections::{Dict, GetElement, Keys, Length, List, Values};
use crate::blocks::control::Pid;
use crate::blocks::logic::{
    And, Equal, GreaterThan, GreaterThanEq, Latch, LessThan, LessThanEq, Not, NotEqual, Or, Xor,
};
use crate::blocks::math::{Abs, Add, ArcTan, Cos, Sub};
use crate::blocks::math::{
    ArcCos, ArcSin, Average, Div, Even, Exp, Log10, Logn, Max, Median, Min, Mod, Mul, Neg, Odd,
    Pow, Sin, Sqrt,
};
use crate::blocks::misc::{HasValue, ParseBool, ParseNumber, Random, SineWave};
use crate::blocks::string::{Concat, Replace, StrLen};

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::sync::Mutex;

use super::{BlockImpl, InputImpl};

pub(crate) type DynBlockProps = dyn BlockProps<
    Reader = <InputImpl as InputProps>::Reader,
    Writer = <InputImpl as InputProps>::Writer,
>;
type MapType = BTreeMap<String, BTreeMap<String, BlockEntry>>;
type BlockRegistry = Mutex<MapType>;

/// Register a block in the registry
#[derive(Debug, Clone)]
pub struct BlockEntry {
    pub desc: BlockDesc,
    pub make: Option<fn() -> Box<DynBlockProps>>,
}

/// Macro for statically registering all the blocks that are
/// available in the system.
#[macro_export]
macro_rules! register_blocks{
    ( $( $block_name:ty ),* ) => {
		lazy_static! {
			/// The block registry
			/// This is a static variable that is initialized once and then
			/// used throughout the lifetime of the program.
			static ref BLOCKS: BlockRegistry = {
				let mut reg = BTreeMap::new();

				$(
					register_impl::<$block_name>(&mut reg);
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
					stringify!($block_name) => {
						let block = <$block_name>::new();
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
					stringify!($block_name) => {
						let block = <$block_name>::new_uuid(uuid);
						eng.schedule(block);
						Ok(uuid)
					}
				)*
				_ => {
					return Err(anyhow!("Block not found"));
				}
			}

		}

		/// Evaluate a static registered block by name.
		/// This will create a block instance and execute it.
		///
		/// # Arguments
		/// - name: The name of the block to evaluate
		/// - inputs: The input values to the block
		///
		/// # Returns
		/// A list of values representing the outputs of the block
		pub async fn eval_static_block(name: &str, inputs: Vec<Value>) -> Result<Vec<Value>> {
			match name {
				$(
					stringify!($block_name) => {
						let mut block = <$block_name>::new();
						eval_block_impl(&mut block, inputs).await
					}
				)*
				_ => {
					return Err(anyhow!("Block not found"));
				}
			}
		}
    };
}

register_blocks!(
    // Logic blocks
    And,
    Or,
    Not,
    Equal,
    NotEqual,
    Xor,
    GreaterThan,
    GreaterThanEq,
    Latch,
    LessThan,
    LessThanEq,
    // Math blocks
    Abs,
    Add,
    ArcCos,
    ArcTan,
    Average,
    Median,
    Even,
    Odd,
    Sub,
    Mul,
    Div,
    Exp,
    Cos,
    ArcSin,
    Sin,
    Log10,
    Logn,
    Sqrt,
    Pow,
    Mod,
    Min,
    Max,
    Neg,
    // Bitwise blocks
    BitwiseAnd,
    BitwiseNot,
    BitwiseOr,
    BitwiseXor,
    // Control blocks
    Pid,
    // String blocks
    Concat,
    Replace,
    StrLen,
    // Collections blocks
    GetElement,
    Length,
    Keys,
    Values,
    List,
    Dict,
    // Misc blocks
    Random,
    SineWave,
    ParseBool,
    ParseNumber,
    HasValue
);

/// Construct a block properties from the registry
/// # Arguments
/// - name: The name of the block to get
/// # Returns
/// A boxed block
pub fn make(name: &str, lib: Option<String>) -> Option<Box<DynBlockProps>> {
    let entry = get_block(name, lib)?;
    entry.make.map(|make| make())
}

/// Get a block entry from the registry
pub fn get_block(name: &str, lib: Option<String>) -> Option<BlockEntry> {
    let reg = BLOCKS.lock().expect("Block registry is locked");
    let lib = lib.unwrap_or_else(|| "core".to_string());

    let reg = reg.get(&lib)?;
    reg.get(name).cloned()
}

/// Get a core block
pub fn get_core_block(name: &str) -> Option<BlockEntry> {
    get_block(name, Some("core".to_string()))
}

/// Get all block descriptions from the registry
pub fn list_registered_blocks() -> Vec<BlockDesc> {
    let reg = BLOCKS.lock().expect("Block registry is locked");

    let mut blocks = Vec::new();
    for (_, lib) in reg.iter() {
        for (_, block) in lib.iter() {
            blocks.push(block.desc.clone());
        }
    }

    blocks
}

/// Register a block with the registry
pub fn register_block_desc(desc: &BlockDesc) -> Result<()> {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    let lib = desc.library.clone();
    let reg = reg.entry(lib).or_default();

    let name = desc.name.clone();
    if reg.contains_key(&name) {
        return Err(anyhow!("Block already registered"));
    }

    reg.insert(
        name.to_string(),
        BlockEntry {
            desc: desc.clone(),
            make: None,
        },
    );

    Ok(())
}

/// Register a block with the registry
/// # Arguments
/// - B: The block type to register
/// # Panics
/// Panics if the block registry is already locked
pub fn register<B: BlockImpl>() {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    register_impl::<B>(&mut reg);
}

/// Evaluate a block directly
///
/// # Arguments
/// - block: The block to evaluate
/// - inputs: The input values to the block
/// # Returns
/// A list of values representing the outputs of the block
pub async fn eval_block_impl<B: BlockImpl>(
    block: &mut B,
    inputs: Vec<Value>,
) -> Result<Vec<Value>> {
    for (i, input) in inputs.iter().enumerate() {
        let mut input_pins = block.inputs_mut();

        if i >= input_pins.len() {
            return Err(anyhow!("Too many inputs"));
        }

        input_pins[i].increment_conn();
        if input_pins[i].writer().try_send(input.clone()).is_ok() && i < inputs.len() - 1 {
            block.read_inputs().await;
        }
    }

    block.execute().await;
    Ok(block.outputs().iter().map(|o| o.value().clone()).collect())
}

fn register_impl<B: BlockImpl>(reg: &mut MapType) {
    let desc = <B as BlockStaticDesc>::desc();
    let lib = desc.library.clone();

    reg.entry(lib).or_default().insert(desc.name.clone(), {
        let make = || -> Box<DynBlockProps> {
            let block = B::default();
            Box::new(block)
        };

        BlockEntry {
            desc: desc.clone(),
            make: Some(make),
        }
    });
}

#[cfg(test)]
mod test {

    use crate::base::block::connect::connect_output;

    use super::*;

    #[test]
    fn test_registry() {
        let add = get_core_block("Add").expect("Add block not found");
        let random = get_core_block("Random").expect("Random block not found");
        let sine = get_core_block("SineWave").expect("SineWave block not found");

        assert_eq!(add.desc.name, "Add");
        assert_eq!(random.desc.name, "Random");
        assert_eq!(sine.desc.name, "SineWave");

        let mut random = random.make.unwrap()();
        let mut outs = random.outputs_mut();

        let mut add = add.make.unwrap()();
        let mut ins = add.inputs_mut();

        let out = outs.first_mut().unwrap();
        let input = ins.first_mut().unwrap();

        connect_output(*out, *input).unwrap();

        let mut eng = crate::single_threaded::SingleThreadedEngine::new();

        schedule_block("Add", &mut eng).expect("Block");

        assert!(eng.blocks().iter().any(|b| b.desc().name == "Add"));
    }

    #[tokio::test]
    async fn test_block_eval() {
        let result = eval_static_block("Add", vec![Value::from(1), Value::from(2)]).await;

        assert_eq!(result.unwrap(), vec![Value::from(3)]);
    }
}
