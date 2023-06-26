// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block execution engine
//!

use anyhow::Result;

use super::{
    block::Block,
    program::data::{BlockData, LinkData},
};

pub mod messages;

/// Specifies the interface for an engine
/// that implements the block execution logic.
pub trait Engine {
    /// The transmission type of the blocks
    type Writer;
    /// The reception type of the blocks
    type Reader;

    /// The type used to send messages to/from this engine.
    type Channel: Send + Sync + Clone;

    /// Schedule a block to be executed by this engine.
    /// This operation can be performed while the engine is running.
    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        block: B,
    );

    /// Load the blocks and links into the engine.
    /// This operation should be performed before the engine is run.
    fn load_blocks_and_links(&mut self, blocks: &[BlockData], links: &[LinkData]) -> Result<()>;

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    async fn run(&mut self);

    /// Get a handle to this engines messaging system so external
    /// systems can communicate with this engine once the engine will run.
    ///
    /// # Arguments
    /// - sender_id The sender unique id.
    /// - sender_channel The sender chanel to send notifications from the engine.
    ///
    /// # Returns
    /// A sender chanel that is used to send messages to the engine.
    ///
    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel;
}
