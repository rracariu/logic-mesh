// Copyright (c) 2022-2023, IntriSemantics Corp.

use super::props::BlockDescAccess;
use super::Block;
use crate::base::input::InputProps;
use crate::base::link::{BaseLink, LinkState};

/// Block connection functions
pub trait BlockConnect: BlockDescAccess {
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

///
/// Implements the `BlockConnect` trait for all types
/// that are `Block`s
///
impl<T: Block> BlockConnect for T {
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

        self.outputs_mut()
            .iter_mut()
            .for_each(|out| out.add_link(link.clone()));
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
            input.decrement_conn();
        });
    }
}
