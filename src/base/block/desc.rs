// Copyright (c) 2022-2023, Radu Racariu.

//! Block description types.

use std::fmt::Display;
use std::str::FromStr;

use libhaystack::val::kind::HaystackKind;

use super::BlockProps;
use crate::base::error::BlockDescError;

/// Static description of a block, used to find the block in the library
/// and inspect its inputs and outputs.
///
/// The `BlockProps` derive builds the description of a native block
/// from its attributes. A block written by hand builds one with
/// [`BlockDesc::new`] and the `with_*` methods: the struct is
/// `#[non_exhaustive]`, so it can gain fields without breaking callers.
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
///
/// Describing a block by hand:
///
/// ```
/// use logic_mesh::{HaystackKind, base::block::BlockDesc};
///
/// let desc = BlockDesc::new("Echo", "examples")
///     .with_category("misc")
///     .with_doc("Copies `in` to `out`.")
///     .with_input("in", HaystackKind::Number)
///     .with_output("out", HaystackKind::Number);
///
/// assert_eq!(desc.qname(), "examples::Echo");
/// assert_eq!(desc.dis, "Echo");
/// assert_eq!(desc.inputs[0].name, "in");
/// ```
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
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
    /// The version a description has unless it names one, as the
    /// `BlockProps` derive's `#[ver = "..."]` does.
    pub const DEFAULT_VER: &'static str = "1.0.0";

    /// A description of the native block `name` in `library`, with no
    /// pins, no category and no documentation. It displays as `name`
    /// and has version [`DEFAULT_VER`](Self::DEFAULT_VER).
    pub fn new(name: impl Into<String>, library: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            dis: name.clone(),
            name,
            library: library.into(),
            ver: Self::DEFAULT_VER.to_string(),
            ..Self::default()
        }
    }

    /// Sets the name the block is displayed under.
    pub fn with_dis(mut self, dis: impl Into<String>) -> Self {
        self.dis = dis.into();
        self
    }

    /// Sets the category the block is listed under.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// Sets the block version.
    pub fn with_ver(mut self, ver: impl Into<String>) -> Self {
        self.ver = ver.into();
        self
    }

    /// Sets the block documentation.
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = doc.into();
        self
    }

    /// Adds an input pin, after the inputs added before it.
    pub fn with_input(mut self, name: impl Into<String>, kind: HaystackKind) -> Self {
        self.inputs.push(BlockPin::new(name, kind));
        self
    }

    /// Adds an output pin, after the outputs added before it.
    pub fn with_output(mut self, name: impl Into<String>, kind: HaystackKind) -> Self {
        self.outputs.push(BlockPin::new(name, kind));
        self
    }

    /// Sets how the block is implemented.
    pub fn with_implementation(mut self, implementation: BlockImplementation) -> Self {
        self.implementation = implementation;
        self
    }

    /// Sets the condition under which the block runs.
    pub fn with_run_condition(mut self, run_condition: BlockRunCondition) -> Self {
        self.run_condition = Some(run_condition);
        self
    }

    /// Returns the qualified name of the block.
    pub fn qname(&self) -> String {
        format!("{}::{}", self.library, self.name)
    }
}

/// Provides static access to a block description, without an instance.
///
/// Complements [`BlockProps::desc`], which takes `&self` so a block can
/// be used as a trait object. The `BlockProps` derive implements both
/// from the same description.
///
/// Only registering a block type needs it (see
/// [`RegisterableBlock`](crate::blocks::registry::RegisterableBlock)):
/// scheduling, connecting, saving and loading a block all go through
/// its instance, so a block whose description is only known at runtime,
/// such as a JS block, does not implement it.
///
/// # Examples
///
/// ```
/// use logic_mesh::{base::block::BlockStaticDesc, blocks::math::Add};
///
/// assert_eq!(Add::static_desc().name, "Add");
/// ```
pub trait BlockStaticDesc: BlockProps {
    /// Returns a static reference to the block description.
    fn static_desc() -> &'static BlockDesc
    where
        Self: Sized;
}

/// A block pin, either an input or an output.
///
/// `#[non_exhaustive]`: build one with [`BlockPin::new`].
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BlockPin {
    /// Pin name.
    pub name: String,
    /// Haystack value kind accepted or produced by this pin.
    pub kind: HaystackKind,
}

impl BlockPin {
    /// A pin named `name` that accepts or produces values of `kind`.
    pub fn new(name: impl Into<String>, kind: HaystackKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}

/// The block implementation type.
///
/// `#[non_exhaustive]`, like [`BlockDesc`] that holds it: another kind
/// of implementation can be added without breaking a `match` on it.
///
/// It parses from, and displays as, `"native"` or `"external"`.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::block::BlockImplementation;
///
/// let imp: BlockImplementation = "external".parse()?;
/// assert_eq!(imp, BlockImplementation::External);
/// assert_eq!(imp.to_string(), "external");
/// # Ok::<(), logic_mesh::BlockDescError>(())
/// ```
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BlockImplementation {
    /// A block that is implemented in Rust.
    #[default]
    Native,
    /// A block that is implemented over a FFI interface, such as JavaScript.
    External,
}

impl FromStr for BlockImplementation {
    type Err = BlockDescError;

    fn from_str(implementation: &str) -> Result<Self, Self::Err> {
        match implementation {
            "native" => Ok(BlockImplementation::Native),
            "external" => Ok(BlockImplementation::External),
            _ => Err(BlockDescError::UnknownImplementation {
                value: implementation.to_string(),
            }),
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
///
/// `#[non_exhaustive]`, like [`BlockDesc`] that holds it: another run
/// condition can be added without breaking a `match` on it.
///
/// It parses from, and displays as, `"change"` or `"always"`.
#[derive(Default, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BlockRunCondition {
    /// Runs on change of inputs.
    #[default]
    Change,
    /// Always runs, regardless of inputs.
    Always,
}

impl FromStr for BlockRunCondition {
    type Err = BlockDescError;

    fn from_str(run_condition: &str) -> Result<Self, Self::Err> {
        match run_condition {
            "change" => Ok(BlockRunCondition::Change),
            "always" => Ok(BlockRunCondition::Always),
            _ => Err(BlockDescError::UnknownRunCondition {
                value: run_condition.to_string(),
            }),
        }
    }
}

impl Display for BlockRunCondition {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Must render exactly the strings `FromStr` accepts, so a
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
    use crate::base::error::BlockDescError;

    #[test]
    fn run_condition_display_round_trips_through_from_str() {
        for cond in [BlockRunCondition::Change, BlockRunCondition::Always] {
            let rendered = cond.to_string();
            let parsed: BlockRunCondition = rendered
                .parse()
                .expect("rendered run condition parses back");
            assert_eq!(parsed, cond);
        }
        assert_eq!(BlockRunCondition::Change.to_string(), "change");
        assert_eq!(BlockRunCondition::Always.to_string(), "always");
    }

    /// Regression: the parse error was a bare `String`.
    #[test]
    fn run_condition_error_is_typed() {
        let err = "bogus"
            .parse::<BlockRunCondition>()
            .expect_err("bogus is rejected");
        assert_eq!(
            err,
            BlockDescError::UnknownRunCondition {
                value: "bogus".to_string()
            }
        );
        assert_eq!(err.to_string(), "Invalid run condition: bogus");
    }

    #[test]
    fn implementation_display_round_trips_through_from_str() {
        for imp in [BlockImplementation::Native, BlockImplementation::External] {
            let rendered = imp.to_string();
            let parsed: BlockImplementation = rendered
                .parse()
                .expect("rendered implementation parses back");
            assert_eq!(parsed, imp);
        }
    }

    /// Regression: the parse error was a bare `String`.
    #[test]
    fn implementation_error_is_typed() {
        let err = "bogus"
            .parse::<BlockImplementation>()
            .expect_err("bogus is rejected");
        assert_eq!(
            err,
            BlockDescError::UnknownImplementation {
                value: "bogus".to_string()
            }
        );
        assert_eq!(err.to_string(), "Invalid implementation: bogus");
    }
}
