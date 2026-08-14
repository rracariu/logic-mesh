// Copyright (c) 2022-2023, Radu Racariu.

//! Link types and traits.

use uuid::Uuid;

pub mod base;
pub use base::BaseLink;

/// The current link state. Phase 1 of fault propagation removed the
/// `Error` variant — channel/transport failures now surface as a `Stale`
/// status on the receiving input pin instead (see [`crate::base::Status`]).
/// `Disconnected`/`Connected` track wiring state only.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum LinkState {
    /// The link is disconnected.
    #[default]
    Disconnected,
    /// The link is connected.
    Connected,
}

/// A link creates a connection from a block output to another block's input.
pub trait Link {
    /// Returns the unique link id.
    fn id(&self) -> &Uuid;

    /// Returns the current link state.
    fn state(&self) -> LinkState;

    /// Returns the id of the target block.
    fn target_block_id(&self) -> &Uuid;

    /// Returns the name of the target input.
    fn target_input(&self) -> &str;
}
