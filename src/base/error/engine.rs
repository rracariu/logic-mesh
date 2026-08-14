// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised by an [engine](crate::base::engine::Engine) while
//! scheduling blocks, wiring links and talking to block actor tasks.
//!

use std::fmt;

use thiserror::Error;
use uuid::Uuid;

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
/// The `BlockTaskGone` / `BlockDroppedReply` / `BlockRequestRejected`
/// trio describes the actor round-trip an engine performs for every
/// block operation: the request could not be delivered, it was delivered
/// but never answered, or the block answered with a failure. All three
/// name the block that was addressed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    /// No block instance with this id is scheduled on the engine.
    #[error("Block instance '{id}' not found")]
    BlockInstanceNotFound { id: Uuid },

    /// A block instance id could not be parsed as a UUID.
    #[error("Invalid block uuid '{uuid}'")]
    InvalidBlockUuid {
        uuid: String,
        #[source]
        source: uuid::Error,
    },

    /// The named pin does not exist on the block at this end of the link.
    #[error("{end} pin '{pin}' not found on block '{block}'")]
    PinNotFound {
        end: LinkEnd,
        block: Uuid,
        pin: String,
    },

    /// The block's actor task is no longer running, so the request could
    /// not be delivered.
    #[error("Block '{id}' actor task is gone")]
    BlockTaskGone { id: Uuid },

    /// The block's actor task dropped the reply channel before answering.
    #[error("Block '{id}' actor task dropped the reply")]
    BlockDroppedReply { id: Uuid },

    /// The block's actor task answered the request with a failure.
    #[error("Block actor rejected the request: {0}")]
    BlockRequestRejected(String),

    /// The multi-threaded engine cannot schedule through the
    /// [`Engine`](crate::base::engine::Engine) trait, whose signature
    /// cannot express the required `Send` bound.
    #[error(
        "MultiThreadedEngine cannot schedule through the `Engine` trait \
         (requires `Send`); use the inherent `schedule_send` method or \
         the `*_send` registry entry points instead"
    )]
    ScheduleRequiresSend,
}

/// Parse a block id, tagging a failure with the string that was rejected.
pub(crate) fn parse_block_uuid(uuid: &str) -> Result<Uuid, EngineError> {
    Uuid::try_from(uuid).map_err(|source| EngineError::InvalidBlockUuid {
        uuid: uuid.to_string(),
        source,
    })
}

#[cfg(test)]
mod test {
    use super::{EngineError, LinkEnd, parse_block_uuid};
    use assert_matches::assert_matches;
    use uuid::Uuid;

    #[test]
    fn invalid_uuid_keeps_the_rejected_string() {
        let err = parse_block_uuid("not-a-uuid").expect_err("should be rejected");

        assert_matches!(err, EngineError::InvalidBlockUuid { uuid, .. } if uuid == "not-a-uuid");
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
