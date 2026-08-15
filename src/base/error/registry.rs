// Copyright (c) 2022-2026, Radu Racariu.

//!
//! Errors raised when looking up, registering or evaluating a block
//! through the [block registry](crate::blocks::registry).
//!

use thiserror::Error;

/// Failures of the block registry: name resolution, registration and
/// direct block evaluation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// No block with this name is registered in the given library.
    #[error("Block '{name}' not found in library '{library}'")]
    BlockNotFound {
        /// The library that was searched.
        library: String,
        /// The block name that was not found.
        name: String,
    },

    /// No block with this name is registered in any library.
    #[error("Block '{name}' not found in the registry")]
    BlockNotRegistered {
        /// The block name that was not found.
        name: String,
    },

    /// An unqualified lookup matched the same block name in more than
    /// one library. Retry the lookup with an explicit library.
    #[error(
        "Block name '{name}' is ambiguous across libraries: {}",
        .libraries.iter().map(|lib| format!("'{lib}'")).collect::<Vec<_>>().join(", ")
    )]
    AmbiguousBlockName {
        /// The ambiguous block name.
        name: String,
        /// The libraries that contain a block with this name.
        libraries: Vec<String>,
    },

    /// A block with this name is already present in the library.
    #[error("Block '{name}' is already registered in library '{library}'")]
    BlockAlreadyRegistered {
        /// The library the block belongs to.
        library: String,
        /// The block name that collided.
        name: String,
    },

    /// More input values were supplied to a block evaluation than the
    /// block declares inputs.
    #[error("Block declares {declared} inputs, but {supplied} values were supplied")]
    TooManyInputs {
        /// The number of inputs the block declares.
        declared: usize,
        /// The number of values that were supplied.
        supplied: usize,
    },
}

#[cfg(test)]
mod test {
    use super::RegistryError;

    #[test]
    fn ambiguous_block_name_lists_libraries() {
        let err = RegistryError::AmbiguousBlockName {
            name: "Add".to_string(),
            libraries: vec!["core".to_string(), "custom".to_string()],
        };

        assert_eq!(
            err.to_string(),
            "Block name 'Add' is ambiguous across libraries: 'core', 'custom'"
        );
    }
}
