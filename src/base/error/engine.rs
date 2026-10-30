// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised by an [engine](crate::base::engine::Engine) while
//! scheduling blocks, wiring links and talking to block actor tasks.
//!

use std::fmt;

use thiserror::Error;
use uuid::Uuid;

use super::{ConnectError, ConnectorError};

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

    /// The multi-threaded engine was asked to schedule a block outside
    /// a tokio runtime, which it spawns every block's actor task onto.
    /// Schedule from within the runtime that will run the engine.
    #[error("No tokio runtime to spawn the actor of block '{id}' on")]
    NoRuntime {
        /// The id of the block that was not scheduled.
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

    /// A program is in a newer format than this crate reads. Nothing of
    /// it is loaded.
    #[error("Program format version {found} is not supported; the newest supported is {supported}")]
    UnsupportedProgramVersion {
        /// The program's [`version`](crate::base::program::Program::version).
        found: u32,
        /// The newest version this crate reads,
        /// [`Program::FORMAT_VERSION`](crate::base::program::Program::FORMAT_VERSION).
        supported: u32,
    },

    /// A program's block was saved at a version of its block type that is
    /// incompatible with the registered one: a different major version,
    /// or below 1.0 a different minor version. Its saved pins and values
    /// may not mean the same to the registered block, so the program is
    /// not loaded.
    #[error(
        "Block '{block}' ({name}) was saved at version {saved}, incompatible with the registered version {registered}"
    )]
    BlockVersionMismatch {
        /// The block's id in the program.
        block: Uuid,
        /// The block type name.
        name: String,
        /// The version the program saved,
        /// [`ProgramBlock::ver`](crate::base::program::ProgramBlock::ver).
        saved: String,
        /// The registered block type's version.
        registered: String,
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

    /// A running engine was asked to attach a connector it manages
    /// already.
    #[error("Connector '{name}' is already managed by the engine")]
    ConnectorAlreadyManaged {
        /// The connector name.
        name: String,
    },

    /// A running engine was asked to detach a connector it does not
    /// manage.
    #[error("Connector '{name}' is not managed by the engine")]
    ConnectorNotManaged {
        /// The connector name.
        name: String,
    },

    /// A connector a running engine was asked to attach failed to
    /// start. It stays registered, so the attach can be retried.
    #[error("Connector '{name}' failed to start")]
    ConnectorStartFailed {
        /// The connector name.
        name: String,
        /// The connector's start failure.
        #[source]
        source: ConnectorError,
    },

    /// A connector a running engine was asked to attach did not start
    /// in time. It stays registered, so the attach can be retried.
    #[error("Connector '{name}' did not start within {millis} ms")]
    ConnectorStartTimeout {
        /// The connector name.
        name: String,
        /// The deadline that elapsed, in milliseconds.
        millis: u64,
    },

    /// The engine is gone: a request to it could not be sent, or it
    /// dropped the request's response channel before answering. An
    /// [`EngineClient`](crate::base::engine::EngineClient) reports it
    /// once the engine has been dropped.
    #[error("The engine is gone")]
    EngineGone,

    /// An engine answered a request with the response to another kind
    /// of request: the transport between them mixed responses up.
    #[error("A {request} request was answered with a {response} response")]
    UnexpectedResponse {
        /// The name of the request's kind,
        /// [`EngineRequest::name`](crate::base::engine::messages::EngineRequest::name).
        request: &'static str,
        /// The name of the response's kind,
        /// [`EngineResponse::name`](crate::base::engine::messages::EngineResponse::name).
        response: &'static str,
    },

    /// A block type evaluated with
    /// [`EngineRequest::EvalBlock`](crate::base::engine::messages::EngineRequest::EvalBlock)
    /// panicked. The panic is caught, as a scheduled block's is, so the
    /// engine goes on answering requests.
    #[error("Evaluating block '{name}' of library '{library}' panicked: {message}")]
    EvalPanicked {
        /// The library of the block type.
        library: String,
        /// The block type name.
        name: String,
        /// The message the block panicked with.
        message: String,
    },

    /// A block type evaluated with
    /// [`EngineRequest::EvalBlock`](crate::base::engine::messages::EngineRequest::EvalBlock)
    /// did not finish in time — typically one that waits for an input
    /// the evaluation did not supply. The evaluation is dropped, so the
    /// engine goes on answering requests.
    #[error("Evaluating block '{name}' of library '{library}' did not finish within {millis} ms")]
    EvalTimeout {
        /// The library of the block type.
        library: String,
        /// The block type name.
        name: String,
        /// The deadline that elapsed, in milliseconds.
        millis: u64,
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
