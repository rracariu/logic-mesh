// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block description
//!

use libhaystack::val::kind::HaystackKind;

use super::BlockProps;

/// Description of a block.
/// This is a static description of a block,
/// used to find the block in the library and
/// inspect its inputs and outputs.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockDesc {
    /// The block name
    pub name: String,
    /// The block library
    pub library: String,
    /// The block friendly name
    pub dis: String,
    /// The block category
    pub category: String,
    /// The block version
    pub ver: String,
    /// List of the inputs of the block
    pub inputs: Vec<BlockPin>,
    /// The outputs of the block
    pub outputs: Vec<BlockPin>,
    /// Block documentation
    pub doc: String,
}

impl BlockDesc {
    /// Returns the qualified name of the block
    pub fn qname(&self) -> String {
        format!("{}::{}", self.library, self.name)
    }
}

/// Trait for providing static access to a block description.
///
/// This would complement the instance method access, as the instance
/// one allows block to be trait objects.
pub trait BlockStaticDesc: BlockProps {
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
