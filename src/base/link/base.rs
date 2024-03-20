// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the base link type.
//!

use uuid::Uuid;

use super::{Link, LinkState};

/// Base Link that uses an abstract
/// optional transmitter type `Tx`.
///
/// Links connect a block output to another block's input.
/// Or, a block input to another block's input.
/// A block output can have multiple links to multiple block inputs.
/// A block input can have multiple links to multiple block inputs.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BaseLink<Tx> {
    /// Unique link id
    pub id: Uuid,
    /// The target block id
    pub target_block_id: Uuid,
    /// The target input name
    pub target_input: String,
    /// Optional transmitter type
    pub tx: Option<Tx>,
    /// The current link state
    pub state: LinkState,
}

impl<Tx: Clone> Link for BaseLink<Tx> {
    fn id(&self) -> &Uuid {
        &self.id
    }

    fn target_block_id(&self) -> &Uuid {
        &self.target_block_id
    }

    fn target_input(&self) -> &str {
        &self.target_input
    }

    fn state(&self) -> LinkState {
        self.state
    }
}

impl<Tx> BaseLink<Tx> {
    pub fn new(target_block_id: Uuid, target_input: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            target_block_id,
            target_input,
            tx: None,
            state: LinkState::Disconnected,
        }
    }
}
