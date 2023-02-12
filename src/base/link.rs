// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

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

/// Base Link that uses an abstract
/// optional transmitter type `Tx`.
///
/// Links connect a block output to another block's input.
/// A block output can have multiple links to multiple block inputs.
///
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
