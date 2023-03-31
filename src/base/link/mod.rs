// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

pub mod base;
pub use base::BaseLink;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum LinkState {
    #[default]
    Disconnected,
    Connected,
    Error,
}

///
/// A link creates a connection from a block
/// output to another's block input.
///
pub trait Link {
    /// Unique link id
    fn id(&self) -> &Uuid;

    /// Current link state
    fn state(&self) -> LinkState;

    /// The id of the target block
    fn target_block_id(&self) -> &Uuid;

    /// The name of the target input
    fn target_input(&self) -> &str;
}
