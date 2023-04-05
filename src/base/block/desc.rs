// Copyright (c) 2022-2023, IntriSemantics Corp.

use super::props::BlockPin;

/// Description of a block.
/// This is a static description of a block,
/// used to find the block in the library and
/// inspect its inputs and outputs.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockDesc {
    /// The block kind
    pub kind: String,
    /// The block library
    pub library: String,
    /// List of the inputs of the block
    pub inputs: Vec<BlockPin>,
    /// The outputs of the block
    pub outputs: Vec<BlockPin>,
    /// Block documentation
    pub doc: String,
}
