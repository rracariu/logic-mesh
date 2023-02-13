// Copyright (c) 2022-2023, IntriSemantics Corp.

use super::input::{Input, InputProps};
use super::link::Link;
use super::output::Output;
use crate::base::link::{BaseLink, LinkState};
use libhaystack::val::kind::HaystackKind;
use uuid::Uuid;

/// Determines the state a block is in
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    #[default]
    Stopped,
    Running,
    Fault,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockMember {
    pub name: String,
    pub kind: HaystackKind,
}

/// Contains information about the block
/// Determines the state a block is in
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockDesc {
    /// The block name
    pub name: String,
    /// The block library
    pub library: String,
    /// List of the inputs of the block
    pub inputs: Vec<BlockMember>,
    /// The output of the block
    pub output: BlockMember,
}

///
/// Defines the the Block properties
///
pub trait BlockProps {
    type Rx;
    type Tx: Clone;

    /// Access the blocks uuid
    fn id(&self) -> &Uuid;

    /// Access the block description
    fn desc() -> &'static BlockDesc;

    /// Blocks state
    fn state(&self) -> BlockState;

    /// Set the blocks state
    fn set_state(&mut self, state: BlockState) -> BlockState;

    /// List all the block inputs
    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>>;

    /// List all the block inputs
    fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>>;

    /// The block output
    fn output(&self) -> &dyn Output<Tx = Self::Tx>;

    /// Mutable reference to the block's output
    fn output_mut(&mut self) -> &mut dyn Output<Tx = Self::Tx>;
}

/// Block connection functions
pub trait BlockConnect: BlockProps {
    /// List all the links this block has
    fn links(&self) -> Vec<&dyn Link>;

    /// Remove a link from the link collection
    fn remove_link(&mut self, link: &dyn Link);

    /// Connect this block to the given input
    ///
    /// # Arguments
    /// - input: The block input to be connected
    ///
    fn connect<I: InputProps<Tx = Self::Tx> + ?Sized>(&mut self, input: &mut I);

    /// Disconnect this block from the given input
    /// # Arguments
    /// - input: The block input to be disconnected
    ///
    fn disconnect<I: InputProps<Tx = Self::Tx>>(&mut self, input: &mut I);
}

pub trait Block: BlockProps + BlockConnect {
    async fn execute(&mut self);
}

///
/// Implements the `BlockConnect` trait for all types
/// that are `Block`s
///
impl<T: Block> BlockConnect for T {
    fn links(&self) -> Vec<&dyn Link> {
        self.output().links()
    }

    fn remove_link(&mut self, link: &dyn Link) {
        self.output_mut().remove_link(link)
    }

    fn connect<I: InputProps<Tx = Self::Tx> + ?Sized>(&mut self, input: &mut I) {
        // Don't connect to itself
        if input.block_id() == self.id() {
            return;
        }

        // Ignore connections to the same block and the same input.
        if self.links().iter().any(|link| {
            link.target_block_id() == input.block_id() && link.target_input() == input.name()
        }) {
            return;
        }

        let mut link = BaseLink::<Self::Tx>::new(*input.block_id(), input.name().to_string());

        link.tx = Some(input.writer().clone());

        link.state = LinkState::Connected;

        self.output_mut().add_link(link);
        input.increment_conn();
    }

    fn disconnect<I: InputProps<Tx = Self::Tx>>(&mut self, input: &mut I) {
        if input.block_id() == self.id() {
            return;
        }

        let links = self
            .links()
            .iter()
            .enumerate()
            .filter(|(_, link)| {
                link.target_input() == input.name() && link.target_block_id() == input.block_id()
            })
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();

        links.into_iter().for_each(|index| {
            self.links().remove(index);
            input.increment_conn();
        });
    }
}
