// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Stable, machine-readable codes for the errors this crate reports.
//!

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    ConnectError, ConnectorError, EngineError, Error, ExternalError, RegistryError, ValueError,
};

/// Declares [`ErrorCode`] with its variants, and the name each one is
/// written as.
macro_rules! error_codes {
    ($($(#[$doc:meta])* $code:ident,)*) => {
        /// A stable, machine-readable name for a failure: one per variant
        /// of the crate's errors, plus the failures only a
        /// [`RequestError`](super::RequestError) reports.
        ///
        /// A code is written as its variant name — `BlockNotFound`,
        /// `DuplicateBlockId`, … — by [`as_str`](Self::as_str), by
        /// `Display` and by serde, and that name never changes for a
        /// code. Where two errors mean the same failure they share a
        /// code: a missing output is `OutputNotFound` whether the engine
        /// or the block reports it.
        ///
        /// `#[non_exhaustive]`: new failures get new codes in minor
        /// releases, so a `match` on it needs a wildcard arm.
        ///
        /// # Examples
        ///
        /// ```
        /// use logic_mesh::{EngineError, ErrorCode, Uuid};
        ///
        /// let err = EngineError::BlockInstanceNotFound { id: Uuid::nil() };
        /// assert_eq!(err.code(), ErrorCode::BlockInstanceNotFound);
        /// assert_eq!(err.code().as_str(), "BlockInstanceNotFound");
        /// ```
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[non_exhaustive]
        pub enum ErrorCode {
            $($(#[$doc])* $code,)*
        }

        impl ErrorCode {
            /// Every code, in declaration order.
            pub const ALL: &'static [ErrorCode] = &[$(ErrorCode::$code,)*];

            /// The code's name, as it is written on the wire.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(ErrorCode::$code => stringify!($code),)*
                }
            }
        }
    };
}

error_codes! {
    // --- Registry -------------------------------------------------------
    /// [`RegistryError::BlockNotFound`].
    BlockNotFound,
    /// [`RegistryError::BlockNotRegistered`].
    BlockNotRegistered,
    /// [`RegistryError::AmbiguousBlockName`].
    AmbiguousBlockName,
    /// [`RegistryError::BlockAlreadyRegistered`].
    BlockAlreadyRegistered,
    /// [`RegistryError::TooManyInputs`].
    TooManyInputs,

    // --- Engine ---------------------------------------------------------
    /// [`EngineError::BlockInstanceNotFound`].
    BlockInstanceNotFound,
    /// [`EngineError::DuplicateBlockId`].
    DuplicateBlockId,
    /// [`EngineError::NoRuntime`].
    NoRuntime,
    /// [`EngineError::InvalidBlockUuid`].
    InvalidBlockUuid,
    /// [`EngineError::PinNotFound`].
    PinNotFound,
    /// [`EngineError::InputNotFound`] and [`ConnectError::InputNotFound`].
    InputNotFound,
    /// [`EngineError::OutputNotFound`] and [`ConnectError::OutputNotFound`].
    OutputNotFound,
    /// [`EngineError::LinkBlockNotInProgram`].
    LinkBlockNotInProgram,
    /// [`EngineError::DuplicateLink`].
    DuplicateLink,
    /// [`EngineError::DuplicateLinkId`].
    DuplicateLinkId,
    /// [`EngineError::InvalidLinkUuid`].
    InvalidLinkUuid,
    /// [`EngineError::UnsupportedProgramVersion`].
    UnsupportedProgramVersion,
    /// [`EngineError::BlockVersionMismatch`].
    BlockVersionMismatch,
    /// [`EngineError::BlockTaskGone`].
    BlockTaskGone,
    /// [`EngineError::BlockDroppedReply`].
    BlockDroppedReply,
    /// [`EngineError::ConnectorAlreadyManaged`].
    ConnectorAlreadyManaged,
    /// [`EngineError::ConnectorNotManaged`].
    ConnectorNotManaged,
    /// [`EngineError::ConnectorStartFailed`].
    ConnectorStartFailed,
    /// [`EngineError::ConnectorStartTimeout`].
    ConnectorStartTimeout,
    /// [`EngineError::EngineGone`].
    EngineGone,
    /// [`EngineError::UnexpectedResponse`].
    UnexpectedResponse,

    // --- Connect --------------------------------------------------------
    /// [`ConnectError::AlreadyConnected`].
    AlreadyConnected,
    /// [`ConnectError::SelfLink`].
    SelfLink,
    /// [`ConnectError::NotConnected`].
    NotConnected,

    // --- Value ----------------------------------------------------------
    /// [`ValueError::KindConversion`].
    KindConversion,
    /// [`ValueError::UnexpectedValue`].
    UnexpectedValue,
    /// [`ValueError::Conversion`].
    ValueConversion,
    /// [`ValueError::UnitConversion`].
    UnitConversion,
    /// [`ValueError::ZincDecode`].
    ZincDecode,
    /// [`ValueError::ZincEncode`].
    ZincEncode,
    /// [`ValueError::ParseBool`].
    ParseBool,

    // --- External -------------------------------------------------------
    /// [`ExternalError::Unsupported`].
    ExternalUnsupported,
    /// [`ExternalError::UnsupportedMultiThreaded`].
    ExternalUnsupportedMultiThreaded,
    /// [`ExternalError::LibraryNotFound`].
    ExternalLibraryNotFound,
    /// [`ExternalError::FactoryCallFailed`].
    ExternalFactoryCallFailed,
    /// [`ExternalError::FactoryReturnedNonFunction`].
    ExternalFactoryReturnedNonFunction,
    /// [`ExternalError::ConnectorPropertyUnreadable`].
    ConnectorPropertyUnreadable,
    /// [`ExternalError::ConnectorMethodMissing`].
    ConnectorMethodMissing,
    /// [`ExternalError::ConnectorMethodNotFunction`].
    ConnectorMethodNotFunction,

    // --- Connector ------------------------------------------------------
    /// [`ConnectorError::NotFound`].
    ConnectorNotFound,
    /// [`ConnectorError::AlreadyRegistered`].
    ConnectorAlreadyRegistered,
    /// [`ConnectorError::Subscribe`].
    ConnectorSubscribeFailed,
    /// [`ConnectorError::Publish`].
    ConnectorPublishFailed,
    /// [`ConnectorError::Request`].
    ConnectorRequestFailed,
    /// [`ConnectorError::Timeout`].
    ConnectorTimeout,
    /// [`ConnectorError::Transport`].
    ConnectorTransport,

    // --- Requests -------------------------------------------------------
    /// A program handed to an engine through a transport — such as a
    /// JavaScript object — is not a program: it does not deserialize.
    InvalidProgram,
    /// An argument handed to an engine through a transport is unusable,
    /// such as a watch id that is not one.
    InvalidArgument,
    /// A transport could not convert a response for its caller.
    Serialization,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Error {
    /// The [`ErrorCode`] of the subsystem error this wraps.
    pub fn code(&self) -> ErrorCode {
        match self {
            Error::Registry(err) => err.code(),
            Error::Engine(err) => err.code(),
            Error::Connect(err) => err.code(),
            Error::Value(err) => err.code(),
            Error::External(err) => err.code(),
            Error::Connector(err) => err.code(),
        }
    }
}

impl RegistryError {
    /// This error's [`ErrorCode`].
    pub fn code(&self) -> ErrorCode {
        match self {
            RegistryError::BlockNotFound { .. } => ErrorCode::BlockNotFound,
            RegistryError::BlockNotRegistered { .. } => ErrorCode::BlockNotRegistered,
            RegistryError::AmbiguousBlockName { .. } => ErrorCode::AmbiguousBlockName,
            RegistryError::BlockAlreadyRegistered { .. } => ErrorCode::BlockAlreadyRegistered,
            RegistryError::TooManyInputs { .. } => ErrorCode::TooManyInputs,
        }
    }
}

impl EngineError {
    /// This error's [`ErrorCode`]. A [`Connect`](Self::Connect) failure
    /// reports the code of the [`ConnectError`] it wraps.
    pub fn code(&self) -> ErrorCode {
        match self {
            EngineError::BlockInstanceNotFound { .. } => ErrorCode::BlockInstanceNotFound,
            EngineError::DuplicateBlockId { .. } => ErrorCode::DuplicateBlockId,
            EngineError::NoRuntime { .. } => ErrorCode::NoRuntime,
            EngineError::InvalidBlockUuid { .. } => ErrorCode::InvalidBlockUuid,
            EngineError::PinNotFound { .. } => ErrorCode::PinNotFound,
            EngineError::InputNotFound { .. } => ErrorCode::InputNotFound,
            EngineError::OutputNotFound { .. } => ErrorCode::OutputNotFound,
            EngineError::LinkBlockNotInProgram { .. } => ErrorCode::LinkBlockNotInProgram,
            EngineError::DuplicateLink { .. } => ErrorCode::DuplicateLink,
            EngineError::DuplicateLinkId { .. } => ErrorCode::DuplicateLinkId,
            EngineError::InvalidLinkUuid { .. } => ErrorCode::InvalidLinkUuid,
            EngineError::UnsupportedProgramVersion { .. } => ErrorCode::UnsupportedProgramVersion,
            EngineError::BlockVersionMismatch { .. } => ErrorCode::BlockVersionMismatch,
            EngineError::BlockTaskGone { .. } => ErrorCode::BlockTaskGone,
            EngineError::BlockDroppedReply { .. } => ErrorCode::BlockDroppedReply,
            EngineError::ConnectorAlreadyManaged { .. } => ErrorCode::ConnectorAlreadyManaged,
            EngineError::ConnectorNotManaged { .. } => ErrorCode::ConnectorNotManaged,
            EngineError::ConnectorStartFailed { .. } => ErrorCode::ConnectorStartFailed,
            EngineError::ConnectorStartTimeout { .. } => ErrorCode::ConnectorStartTimeout,
            EngineError::EngineGone => ErrorCode::EngineGone,
            EngineError::UnexpectedResponse { .. } => ErrorCode::UnexpectedResponse,
            EngineError::Connect(err) => err.code(),
        }
    }
}

impl ConnectError {
    /// This error's [`ErrorCode`].
    pub fn code(&self) -> ErrorCode {
        match self {
            ConnectError::OutputNotFound { .. } => ErrorCode::OutputNotFound,
            ConnectError::InputNotFound { .. } => ErrorCode::InputNotFound,
            ConnectError::AlreadyConnected { .. } => ErrorCode::AlreadyConnected,
            ConnectError::SelfLink { .. } => ErrorCode::SelfLink,
            ConnectError::NotConnected { .. } => ErrorCode::NotConnected,
        }
    }
}

impl ValueError {
    /// This error's [`ErrorCode`].
    pub fn code(&self) -> ErrorCode {
        match self {
            ValueError::KindConversion { .. } => ErrorCode::KindConversion,
            ValueError::UnexpectedValue { .. } => ErrorCode::UnexpectedValue,
            ValueError::Conversion(_) => ErrorCode::ValueConversion,
            ValueError::UnitConversion(_) => ErrorCode::UnitConversion,
            ValueError::ZincDecode(_) => ErrorCode::ZincDecode,
            ValueError::ZincEncode(_) => ErrorCode::ZincEncode,
            ValueError::ParseBool(_) => ErrorCode::ParseBool,
        }
    }
}

impl ExternalError {
    /// This error's [`ErrorCode`].
    pub fn code(&self) -> ErrorCode {
        match self {
            ExternalError::Unsupported => ErrorCode::ExternalUnsupported,
            ExternalError::UnsupportedMultiThreaded => ErrorCode::ExternalUnsupportedMultiThreaded,
            ExternalError::LibraryNotFound { .. } => ErrorCode::ExternalLibraryNotFound,
            ExternalError::FactoryCallFailed { .. } => ErrorCode::ExternalFactoryCallFailed,
            ExternalError::FactoryReturnedNonFunction { .. } => {
                ErrorCode::ExternalFactoryReturnedNonFunction
            }
            ExternalError::ConnectorPropertyUnreadable { .. } => {
                ErrorCode::ConnectorPropertyUnreadable
            }
            ExternalError::ConnectorMethodMissing { .. } => ErrorCode::ConnectorMethodMissing,
            ExternalError::ConnectorMethodNotFunction { .. } => {
                ErrorCode::ConnectorMethodNotFunction
            }
        }
    }
}

impl ConnectorError {
    /// This error's [`ErrorCode`].
    pub fn code(&self) -> ErrorCode {
        match self {
            ConnectorError::NotFound { .. } => ErrorCode::ConnectorNotFound,
            ConnectorError::AlreadyRegistered { .. } => ErrorCode::ConnectorAlreadyRegistered,
            ConnectorError::Subscribe { .. } => ErrorCode::ConnectorSubscribeFailed,
            ConnectorError::Publish { .. } => ErrorCode::ConnectorPublishFailed,
            ConnectorError::Request { .. } => ErrorCode::ConnectorRequestFailed,
            ConnectorError::Timeout { .. } => ErrorCode::ConnectorTimeout,
            ConnectorError::Transport { .. } => ErrorCode::ConnectorTransport,
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use super::ErrorCode;

    /// A code is written as its variant name, and no two codes share a
    /// name.
    #[test]
    fn codes_are_written_as_their_unique_names() {
        let mut names = BTreeSet::new();
        for code in ErrorCode::ALL {
            assert_eq!(code.to_string(), code.as_str());
            assert_eq!(
                serde_json::to_value(code).ok(),
                Some(serde_json::Value::String(code.as_str().to_string()))
            );
            assert!(names.insert(code.as_str()), "{code} is listed twice");
        }
    }

    /// The TypeScript wrapper types an error's code as a union of the
    /// names; it has to list every one. Skipped where the web sources
    /// are not around, such as a packaged crate.
    #[test]
    fn the_typescript_code_union_lists_every_code() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/web/packages/logic-mesh/src/LogicMeshError.ts"
        );
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        for code in ErrorCode::ALL {
            assert!(
                source.contains(&format!("'{code}'")),
                "{path} does not list the code {code}"
            );
        }
    }
}
