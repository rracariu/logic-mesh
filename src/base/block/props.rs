// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block properties
//!

use libhaystack::val::kind::HaystackKind;
use uuid::Uuid;

use crate::base::{input::Input, link::Link, output::Output};

use super::{desc::BlockDesc, BlockState};

/// Defines the the Block properties
/// that are common to all blocks.
pub trait BlockProps {
    /// The block's read type
    /// This is the type used to read from the block's inputs
    type Read;
    /// The block's write type
    /// This is the type used to write to the block's outputs
    type Write: Clone;

    /// Blocks unique id
    fn id(&self) -> &Uuid;

    /// Blocks name
    fn name(&self) -> &str;

    /// Block's static description
    fn desc(&self) -> &'static BlockDesc;

    /// Blocks state
    fn state(&self) -> BlockState;

    /// Set the blocks state
    fn set_state(&mut self, state: BlockState) -> BlockState;

    /// List all the block inputs
    fn inputs(&self) -> Vec<&dyn Input<Read = Self::Read, Write = Self::Write>>;

    /// List all the block inputs
    fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Read = Self::Read, Write = Self::Write>>;

    /// The block output
    fn outputs(&self) -> Vec<&dyn Output<Write = Self::Write>>;

    /// Mutable reference to the block's output
    fn outputs_mut(&mut self) -> Vec<&mut dyn Output<Write = Self::Write>>;

    /// List all the links this block has
    fn links(&self) -> Vec<&dyn Link>;

    /// Remove a link from the link collection
    fn remove_link(&mut self, link: &dyn Link);
}

/// Trait for providing static access to a block description.
///
/// This would complement the instance method access, as the instance
/// one allows block to be trait objects.
pub trait BlockDescAccess: BlockProps {
    /// Static access to the block description
    fn desc() -> &'static BlockDesc;
}

/// Defines a block pin
/// A block pin is either an input or an output
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockPin {
    pub name: String,
    pub kind: HaystackKind,
}
