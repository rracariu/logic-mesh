// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block properties
//!

use uuid::Uuid;

use crate::base::{input::Input, link::Link, output::Output};

use super::{desc::BlockDesc, BlockState};

/// Defines the the Block properties
/// that are common to all blocks.
pub trait BlockProps {
    /// The block's read type
    /// This is the type used to read from the block's inputs
    type Reader;
    /// The block's write type
    /// This is the type used to write to the block's outputs
    type Writer: Clone;

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
    fn inputs(&self) -> Vec<&dyn Input<Reader = Self::Reader, Writer = Self::Writer>>;

    /// Get block input by name
    fn get_input(
        &self,
        name: &str,
    ) -> Option<&dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs().iter().find(|i| i.name() == name).cloned()
    }

    /// Get block mutable input by name
    fn get_input_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
        self.inputs_mut().into_iter().find(|i| i.name() == name)
    }

    /// List all the block inputs
    fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Reader = Self::Reader, Writer = Self::Writer>>;

    /// The block outputs
    fn outputs(&self) -> Vec<&dyn Output<Writer = Self::Writer>>;

    /// Get block output by name
    fn get_output(&self, name: &str) -> Option<&dyn Output<Writer = Self::Writer>> {
        self.outputs().iter().find(|i| i.name() == name).cloned()
    }

    /// Get block mutable output by name
    fn get_output_mut(&mut self, name: &str) -> Option<&mut dyn Output<Writer = Self::Writer>> {
        self.outputs_mut().into_iter().find(|i| i.name() == name)
    }

    /// Mutable reference to the block's output
    fn outputs_mut(&mut self) -> Vec<&mut dyn Output<Writer = Self::Writer>>;

    /// List all the links this block has
    fn links(&self) -> Vec<(&str, Vec<&dyn crate::base::link::Link>)>;

    /// Remove a link from the link collection
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Remove a link by its id from the link collection
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Remove all links from this block
    fn remove_all_links(&mut self);
}
