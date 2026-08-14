// Copyright (c) 2022-2023, Radu Racariu.

//! Block execution engine.

use super::error::Result;
use super::{block::Block, program::Program};

pub mod messages;

/// Interface for an engine that implements block execution logic.
pub trait Engine {
    /// The transmission type of the blocks.
    type Writer;
    /// The reception type of the blocks.
    type Reader;

    /// The type used to send messages to/from this engine.
    type Channel: Send + Sync + Clone;

    /// Schedules a block to be executed by this engine.
    ///
    /// Engines that cannot schedule through this trait — e.g. the
    /// multi-threaded engine, which needs a [`Send`] bound this signature
    /// cannot express — return an error instead.
    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        block: B,
    ) -> Result<()>;

    /// Synchronously schedules the blocks and validates-and-queues the links
    /// from a [`Program`]. This is the pre-run setup half of program
    /// loading; it does NOT push input/output constant values (which
    /// requires the actor loop to be running — see `load_program` on the
    /// inherent engine impls for the full async path).
    fn schedule_program_blocks(&mut self, program: &Program) -> Result<()>;

    /// Runs the event loop of this engine and executes the scheduled blocks.
    #[allow(async_fn_in_trait)]
    async fn run(&mut self);

    /// Returns a handle to this engine's messaging system so external
    /// systems can communicate with this engine once it is running.
    ///
    /// # Arguments
    ///
    /// * `sender_id` - The sender's unique id.
    /// * `sender_channel` - The sender channel to send notifications from the engine.
    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel;
}
