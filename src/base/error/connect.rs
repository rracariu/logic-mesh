// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when linking block pins together, through
//! [`BlockConnect`](crate::base::block::BlockConnect), the
//! [`connect`](crate::base::block::connect) functions, or an engine
//! wiring a link between two scheduled blocks.
//!
//! Not to be confused with [`ConnectorError`](super::ConnectorError),
//! which reports failures of the connectors that exchange data with
//! external systems.
//!

use thiserror::Error;
use uuid::Uuid;

/// Failures of connecting and disconnecting block pins.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     ConnectError,
///     base::block::{BlockConnect, BlockProps},
///     blocks::math::Add,
/// };
///
/// let mut source = Add::new();
/// let mut target = Add::new();
/// let err = source
///     .connect_output("nope", target.inputs_mut()[0])
///     .expect_err("Add has no `nope` output");
///
/// assert!(matches!(err, ConnectError::OutputNotFound { pin, .. } if pin == "nope"));
/// ```
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectError {
    /// The source block has no output with this name.
    #[error("Output '{pin}' not found on block '{block}'")]
    OutputNotFound {
        /// The block the output was looked up on.
        block: Uuid,
        /// The output name that was not found.
        pin: String,
    },

    /// The block has no input with this name.
    #[error("Input '{pin}' not found on block '{block}'")]
    InputNotFound {
        /// The block the input was looked up on.
        block: Uuid,
        /// The input name that was not found.
        pin: String,
    },

    /// The source pin already links to the target input. A pin links
    /// to an input at most once.
    #[error(
        "Pin '{source_pin}' of block '{source_block}' is already linked to input \
         '{target_input}' of block '{target_block}'"
    )]
    AlreadyConnected {
        /// The block of the source pin.
        source_block: Uuid,
        /// The source pin's name: an output, or an input that fans out.
        source_pin: String,
        /// The block of the target input.
        target_block: Uuid,
        /// The target input's name.
        target_input: String,
    },

    /// An input was linked to an input of its own block. An input fans
    /// out only to inputs of other blocks; an output may link to its
    /// own block's inputs.
    #[error(
        "Input '{source_input}' of block '{block}' cannot link to input '{target_input}' \
         of the same block"
    )]
    SelfLink {
        /// The block both inputs belong to.
        block: Uuid,
        /// The name of the input the link would read from.
        source_input: String,
        /// The name of the input the link would write to.
        target_input: String,
    },

    /// The source pin has no link to the target input, so there is
    /// nothing to disconnect.
    #[error(
        "Pin '{source_pin}' of block '{source_block}' is not linked to input \
         '{target_input}' of block '{target_block}'"
    )]
    NotConnected {
        /// The block of the source pin.
        source_block: Uuid,
        /// The source pin's name: an output, or an input that fans out.
        source_pin: String,
        /// The block of the target input.
        target_block: Uuid,
        /// The target input's name.
        target_input: String,
    },
}

#[cfg(test)]
mod test {
    use super::ConnectError;
    use uuid::Uuid;

    /// Regression: a link failure named only its target input and
    /// called the source "this pin", which means nothing in an engine
    /// reply that carries the message alone.
    #[test]
    fn messages_name_the_pins() {
        let (source, target) = (Uuid::new_v4(), Uuid::new_v4());

        assert_eq!(
            ConnectError::InputNotFound {
                block: target,
                pin: "in9".to_string()
            }
            .to_string(),
            format!("Input 'in9' not found on block '{target}'")
        );
        assert_eq!(
            ConnectError::AlreadyConnected {
                source_block: source,
                source_pin: "out".to_string(),
                target_block: target,
                target_input: "in0".to_string()
            }
            .to_string(),
            format!(
                "Pin 'out' of block '{source}' is already linked to input 'in0' of block '{target}'"
            )
        );
        assert_eq!(
            ConnectError::NotConnected {
                source_block: source,
                source_pin: "in1".to_string(),
                target_block: target,
                target_input: "in0".to_string()
            }
            .to_string(),
            format!(
                "Pin 'in1' of block '{source}' is not linked to input 'in0' of block '{target}'"
            )
        );
        assert_eq!(
            ConnectError::SelfLink {
                block: source,
                source_input: "in0".to_string(),
                target_input: "in1".to_string()
            }
            .to_string(),
            format!("Input 'in0' of block '{source}' cannot link to input 'in1' of the same block")
        );
    }
}
