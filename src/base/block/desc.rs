// Copyright (c) 2022-2023, Radu Racariu.

//! Block description types.

use std::fmt::Display;

use libhaystack::val::kind::HaystackKind;

use super::BlockProps;

/// Static description of a block, used to find the block in the library
/// and inspect its inputs and outputs.
///
/// # Examples
///
/// ```
/// use logic_mesh::blocks::registry::get_block;
///
/// let entry = get_block("Add", Some("core")).expect("Add exists");
/// let desc = &entry.desc;
/// assert_eq!(desc.qname(), "core::Add");
/// assert!(!desc.inputs.is_empty());
/// assert!(!desc.outputs.is_empty());
/// ```
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockDesc {
    /// The block name.
    pub name: String,
    /// The block library.
    pub library: String,
    /// The block friendly name.
    pub dis: String,
    /// The block category.
    pub category: String,
    /// The block version.
    pub ver: String,
    /// Inputs of the block.
    pub inputs: Vec<BlockPin>,
    /// The outputs of the block.
    pub outputs: Vec<BlockPin>,
    /// Block documentation.
    pub doc: String,
    /// Block implementation type.
    pub implementation: BlockImplementation,

    /// The condition under which the block should run.
    pub run_condition: Option<BlockRunCondition>,
}

impl BlockDesc {
    /// Returns the qualified name of the block.
    pub fn qname(&self) -> String {
        format!("{}::{}", self.library, self.name)
    }
}

/// Provides static access to a block description.
///
/// Complements the instance method access, as the instance
/// one allows the block to be a trait object.
pub trait BlockStaticDesc: BlockProps {
    /// Returns a static reference to the block description.
    fn desc() -> &'static BlockDesc
    where
        Self: Sized;
}

/// A block pin, either an input or an output.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct BlockPin {
    /// Pin name.
    pub name: String,
    /// Haystack value kind accepted or produced by this pin.
    pub kind: HaystackKind,
}

/// The block implementation type.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum BlockImplementation {
    /// A block that is implemented in Rust.
    #[default]
    Native,
    /// A block that is implemented over a FFI interface, such as JavaScript.
    External,
}

impl TryFrom<&str> for BlockImplementation {
    type Error = String;

    fn try_from(implementation: &str) -> Result<Self, Self::Error> {
        match implementation {
            "native" => Ok(BlockImplementation::Native),
            "external" => Ok(BlockImplementation::External),
            _ => Err(format!("Invalid implementation: {implementation}")),
        }
    }
}

impl Display for BlockImplementation {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            BlockImplementation::Native => "native",
            BlockImplementation::External => "external",
        };
        write!(fmt, "{kind}")
    }
}

/// The condition under which a block should run.
#[derive(Default, Debug, Clone, PartialEq)]
pub enum BlockRunCondition {
    /// Runs on change of inputs.
    #[default]
    Change,
    /// Always runs, regardless of inputs.
    Always,
}

impl TryFrom<&str> for BlockRunCondition {
    type Error = String;

    fn try_from(run_condition: &str) -> Result<Self, Self::Error> {
        match run_condition {
            "change" => Ok(BlockRunCondition::Change),
            "always" => Ok(BlockRunCondition::Always),
            _ => Err(format!("Invalid run condition: {run_condition}")),
        }
    }
}

impl Display for BlockRunCondition {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Must render exactly the strings `TryFrom<&str>` accepts, so a
        // listed description round-trips back through registration.
        let kind = match self {
            BlockRunCondition::Change => "change",
            BlockRunCondition::Always => "always",
        };
        write!(fmt, "{kind}")
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockImplementation, BlockRunCondition};

    #[test]
    fn run_condition_display_round_trips_through_try_from() {
        for cond in [BlockRunCondition::Change, BlockRunCondition::Always] {
            let rendered = cond.to_string();
            let parsed = BlockRunCondition::try_from(rendered.as_str())
                .expect("rendered run condition parses back");
            assert_eq!(parsed, cond);
        }
        assert_eq!(BlockRunCondition::Change.to_string(), "change");
        assert_eq!(BlockRunCondition::Always.to_string(), "always");
    }

    #[test]
    fn run_condition_error_names_the_run_condition() {
        let err = BlockRunCondition::try_from("bogus").expect_err("bogus is rejected");
        assert_eq!(err, "Invalid run condition: bogus");
    }

    #[test]
    fn implementation_display_round_trips_through_try_from() {
        for imp in [BlockImplementation::Native, BlockImplementation::External] {
            let rendered = imp.to_string();
            let parsed = BlockImplementation::try_from(rendered.as_str())
                .expect("rendered implementation parses back");
            assert_eq!(parsed, imp);
        }
    }

    #[test]
    fn implementation_error_names_the_implementation() {
        let err = BlockImplementation::try_from("bogus").expect_err("bogus is rejected");
        assert_eq!(err, "Invalid implementation: bogus");
    }
}
