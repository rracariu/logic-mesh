// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block execution engine
//!

use anyhow::Result;

use super::{block::Block, program::data::LinkData};

pub mod messages;

/// Specifies the interface for an engine
/// that implements the block execution logic.
pub trait Engine {
    /// The transmission type of the blocks
    type Writer;
    /// The reception type of the blocks
    type Reader;

    /// The type of the sender used by this engine.
    type Sender: Send + Sync + Clone;

    /// Schedule a block to be executed by this engine.
    /// This operation can be performed while the engine is running.
    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        block: B,
    );

    /// Connects a block input or output to another
    /// block's input
    fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData>;

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    async fn run(&mut self);

    /// Get a handle to this engines messaging system so external
    /// systems can communicate with this engine once the engine will run.
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
