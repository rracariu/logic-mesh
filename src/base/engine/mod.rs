// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block execution engine
//!

use super::block::{Block, BlockProps};

pub mod messages;

/// Specifies the interface for an engine
/// that implements the block execution logic.
pub trait Engine {
    /// The transmission type of the blocks
    type Tx;
    /// The reception type of the blocks
    type Rx;

    /// The type of the sender used by this engine.
    type Sender: Send + Sync + Clone;

    /// Get a list of all the blocks that are currently
    /// scheduled on this engine.
    fn blocks(&self) -> Vec<&dyn BlockProps<Tx = Self::Tx, Rx = Self::Rx>>;

    /// Schedule a block to be executed by this engine
    fn schedule<B: Block<Tx = Self::Tx, Rx = Self::Rx> + 'static>(&mut self, block: B);

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    async fn run(&mut self);

    /// Get a handle to this engines messaging system so external
    /// systems can communicate with this engine.
    ///
    /// # Arguments
    /// - sender_id The sender unique id.
    /// - sender The sender chanel to send notifications from the engine.
    ///
    /// # Returns
    /// A sender chanel that is used to send messages to the engine.
    ///
    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender: Self::Sender,
    ) -> Self::Sender;
}