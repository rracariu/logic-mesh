// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block execution engine
//!

use anyhow::Result;

use super::{block::Block, program::Program};

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

    /// Synchronously schedule the blocks and validate-and-queue the links
    /// from a [`Program`]. This is the pre-run setup half of program
    /// loading; it does NOT push input/output constant values (which
    /// requires the actor loop to be running — see `load_program` on the
    /// inherent engine impls for the full async path).
    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()>;

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    #[allow(async_fn_in_trait)]
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
