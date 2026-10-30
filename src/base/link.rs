// Copyright (c) 2022-2023, Radu Racariu.

//! Link types and traits.

use uuid::Uuid;

use super::sealed::Sealed;

pub mod base;
pub use base::BaseLink;

/// The current link state. Phase 1 of fault propagation removed the
/// `Error` variant — channel/transport failures now surface as a
/// [`Status::Stale`](crate::base::Status::Stale) status on the receiving
/// input pin instead (see [`crate::base::Status`]).
/// [`LinkState::Disconnected`]/[`LinkState::Connected`] track wiring state only.
///
/// `#[non_exhaustive]`: further states may follow, so a `match` on it
/// needs a wildcard arm.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum LinkState {
    /// The link is disconnected.
    #[default]
    Disconnected,
    /// The link is connected.
    Connected,
}

/// A link creates a connection from a block output to another block's input.
///
/// # Sealed
///
/// Only [`BaseLink`] implements this trait, and no other crate can, so
/// methods can be added to it in a minor release. A link to a block is
/// made with [`BlockConnect`](crate::base::block::BlockConnect) or an
/// engine message, never by hand:
///
/// ```compile_fail,E0277
/// use logic_mesh::Uuid;
/// use logic_mesh::base::link::{Link, LinkState};
///
/// struct MyLink(Uuid);
///
/// impl Link for MyLink {
///     fn id(&self) -> &Uuid {
///         &self.0
///     }
///     fn state(&self) -> LinkState {
///         LinkState::Connected
///     }
///     fn target_block_id(&self) -> &Uuid {
///         &self.0
///     }
///     fn target_input(&self) -> &str {
///         "in"
///     }
/// }
/// ```
pub trait Link: Sealed {
    /// Returns the unique link id.
    fn id(&self) -> &Uuid;

    /// Returns the current link state.
    fn state(&self) -> LinkState;

    /// Returns the id of the target block.
    fn target_block_id(&self) -> &Uuid;

    /// Returns the name of the target input.
    fn target_input(&self) -> &str;
}
