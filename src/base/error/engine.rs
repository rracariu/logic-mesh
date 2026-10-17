// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised by an [engine](crate::base::engine::Engine) while
//! scheduling blocks, wiring links and talking to block actor tasks.
//!

use std::fmt;

use thiserror::Error;
use uuid::Uuid;

use super::ConnectError;

/// Which end of a link a failure refers to.
///
/// A link is validated at both ends, and for a self-link both ends carry
/// the same block id — so the block id alone cannot say which check
/// rejected the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkEnd {
    /// The pin the link reads from.
    Source,
    /// The input the link writes to.
    Target,
}

impl fmt::Display for LinkEnd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            LinkEnd::Source => "Source",
            LinkEnd::Target => "Target",
        })
    }
}

/// Failures of the block execution engine.
///
/// The `BlockTaskGone` / `BlockDroppedReply` pair describes the actor
/// round-trip an engine performs for every block operation: the request
/// could not be delivered, or it was delivered but never answered. Both
/// name the block that was addressed. A block that answers with a
/// failure reports it in its own terms: a missing pin as
/// `InputNotFound` / `OutputNotFound`, a link it cannot take as
/// `Connect`.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    /// No block instance with this id is scheduled on the engine.
    #[error("Block instance '{id}' not found")]
    BlockInstanceNotFound {
        /// The id of the block that was not found.
        id: Uuid,
    },

    /// A block with this id is already scheduled on the engine. Block
    /// ids are unique per engine, and a scheduled block is never
    /// silently replaced.
    #[error("Block instance '{id}' is already scheduled")]
    DuplicateBlockId {
        /// The id that is already taken.
        id: Uuid,
    },

    /// A block instance id could not be parsed as a UUID.
    #[error("Invalid block uuid '{uuid}'")]
    InvalidBlockUuid {
        /// The string that could not be parsed.
        uuid: String,
        /// The underlying parse error.
        #[source]
        source: uuid::Error,
    },

    /// The named pin does not exist on the block at this end of the link.
    #[error("{end} pin '{pin}' not found on block '{block}'")]
    PinNotFound {
        /// Which end of the link failed validation.
        end: LinkEnd,
        /// The block the pin was looked up on.
        block: Uuid,
        /// The pin name that was not found.
        pin: String,
    },

    /// The named input does not exist on the block.
    #[error("Input '{pin}' not found on block '{block}'")]
    InputNotFound {
        /// The block the input was looked up on.
        block: Uuid,
        /// The input name that was not found.
        pin: String,
    },

    /// The named output does not exist on the block.
    #[error("Output '{pin}' not found on block '{block}'")]
    OutputNotFound {
        /// The block the output was looked up on.
        block: Uuid,
        /// The output name that was not found.
        pin: String,
    },

    /// A program's link names a block that is not part of the same
    /// program. A [`Program`](crate::base::program::Program) loads as a
    /// self-contained unit, so its links only join its own blocks.
    #[error("{end} block '{block}' of link '{link}' is not part of the program")]
    LinkBlockNotInProgram {
        /// The link that failed validation.
        link: Uuid,
        /// Which end of the link names the block.
        end: LinkEnd,
        /// The block id the link names.
        block: Uuid,
    },

    /// Two links of a program join the same two pins. A pin links to an
    /// input at most once.
    #[error("Link '{link}' joins the same pins as link '{existing}'")]
    DuplicateLink {
        /// The link that repeats an earlier one.
        link: Uuid,
        /// The earlier link joining the same pins.
        existing: Uuid,
    },

    /// A link with this id already exists on the engine. Link ids are
    /// unique per engine, like block ids, and an existing link is never
    /// silently replaced.
    #[error("Link '{id}' already exists")]
    DuplicateLinkId {
        /// The id that is already taken.
        id: Uuid,
    },

    /// A link id could not be parsed as a UUID.
    #[error("Invalid link uuid '{uuid}'")]
    InvalidLinkUuid {
        /// The string that could not be parsed.
        uuid: String,
        /// The underlying parse error.
        #[source]
        source: uuid::Error,
    },

    /// The block's actor task is no longer running, so the request could
    /// not be delivered.
    #[error("Block '{id}' actor task is gone")]
    BlockTaskGone {
        /// The id of the unreachable block.
        id: Uuid,
    },

    /// The block's actor task dropped the reply channel before answering.
    #[error("Block '{id}' actor task dropped the reply")]
    BlockDroppedReply {
        /// The id of the block that dropped the reply.
        id: Uuid,
    },

    /// A block rejected a link while the engine wired it, after the
    /// engine's own checks passed — for example an input linked to
    /// another input of its own block.
    #[error(transparent)]
    Connect(#[from] ConnectError),
}

/// Parses a block id, tagging a failure with the string that was rejected.
pub(crate) fn parse_block_uuid(uuid: &str) -> Result<Uuid, EngineError> {
    Uuid::try_from(uuid).map_err(|source| EngineError::InvalidBlockUuid {
        uuid: uuid.to_string(),
        source,
    })
}

/// Parses a link id, tagging a failure with the string that was rejected.
pub(crate) fn parse_link_uuid(uuid: &str) -> Result<Uuid, EngineError> {
    Uuid::try_from(uuid).map_err(|source| EngineError::InvalidLinkUuid {
        uuid: uuid.to_string(),
        source,
    })
}

#[cfg(test)]
mod test {
    use super::{EngineError, LinkEnd, parse_block_uuid, parse_link_uuid};
    use assert_matches::assert_matches;
    use uuid::Uuid;

    #[test]
    fn invalid_uuid_keeps_the_rejected_string() {
        let err = parse_block_uuid("not-a-uuid").expect_err("should be rejected");

        assert_matches!(err, EngineError::InvalidBlockUuid { uuid, .. } if uuid == "not-a-uuid");
    }

    #[test]
    fn invalid_link_uuid_keeps_the_rejected_string() {
        let err = parse_link_uuid("link-1").expect_err("should be rejected");

        assert_matches!(err, EngineError::InvalidLinkUuid { uuid, .. } if uuid == "link-1");
    }

    /// Both ends of a self-link name the same block, so the end is the
    /// only thing telling the two validation failures apart.
    #[test]
    fn pin_not_found_names_the_link_end() {
        let block = Uuid::new_v4();
        let source = EngineError::PinNotFound {
            end: LinkEnd::Source,
            block,
            pin: "x".to_string(),
        };
        let target = EngineError::PinNotFound {
            end: LinkEnd::Target,
            block,
            pin: "x".to_string(),
        };

        assert_eq!(
            source.to_string(),
            format!("Source pin 'x' not found on block '{block}'")
        );
        assert_eq!(
            target.to_string(),
            format!("Target pin 'x' not found on block '{block}'")
        );
    }
}
