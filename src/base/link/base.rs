// Copyright (c) 2022-2023, Radu Racariu.

//! Base link type.

use uuid::Uuid;

use crate::base::sealed::Sealed;

use super::{Link, LinkState};

/// Base link that uses an abstract optional transmitter type `Tx`.
///
/// Links connect a block output to another block's input, or a block
/// input to another block's input. A block output or input can have
/// multiple links to multiple block inputs.
///
/// Its fields are private: read them through [`Link`] and
/// [`tx`](Self::tx). The [`connect_output`](crate::base::block::connect_output)
/// and [`connect_input`](crate::base::block::connect_input) functions and the engines create the links between pins; a pin
/// implementation that wires its own builds one with
/// [`connected`](Self::connected).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BaseLink<Tx> {
    /// Unique link id.
    pub(crate) id: Uuid,
    /// The target block id.
    pub(crate) target_block_id: Uuid,
    /// The target input name.
    pub(crate) target_input: String,
    /// Optional transmitter type.
    pub(crate) tx: Option<Tx>,
    /// The current link state.
    pub(crate) state: LinkState,
}

impl<Tx> Sealed for BaseLink<Tx> {}

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
    /// Creates a new disconnected link targeting the given block and input.
    pub fn new(target_block_id: Uuid, target_input: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            target_block_id,
            target_input,
            tx: None,
            state: LinkState::Disconnected,
        }
    }

    /// Creates the connected link `id` to input `target_input` of
    /// block `target_block_id`, which sends on `tx`.
    pub fn connected(id: Uuid, target_block_id: Uuid, target_input: String, tx: Tx) -> Self {
        Self {
            id,
            target_block_id,
            target_input,
            tx: Some(tx),
            state: LinkState::Connected,
        }
    }

    /// The transmitter this link sends on, if it is connected.
    pub fn tx(&self) -> Option<&Tx> {
        self.tx.as_ref()
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use super::BaseLink;
    use crate::base::link::{Link, LinkState};

    #[test]
    fn connected_link_reports_its_ends_and_transmitter() {
        let (id, target) = (Uuid::new_v4(), Uuid::new_v4());
        let link = BaseLink::connected(id, target, "in0".to_string(), 7u8);

        assert_eq!(link.id(), &id);
        assert_eq!(link.target_block_id(), &target);
        assert_eq!(link.target_input(), "in0");
        assert_eq!(link.state(), LinkState::Connected);
        assert_eq!(link.tx(), Some(&7));
    }

    #[test]
    fn new_link_is_disconnected() {
        let link = BaseLink::<u8>::new(Uuid::new_v4(), "in0".to_string());

        assert_eq!(link.state(), LinkState::Disconnected);
        assert!(link.tx().is_none());
    }
}
