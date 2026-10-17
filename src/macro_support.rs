// Copyright (c) 2022-2026, Radu Racariu.

//! Not public API: the interface between this crate and the code the
//! `#[block]` attribute and the `BlockProps` derive generate.
//!
//! Generated code builds a block's description and pins only through
//! these functions, never through a struct literal or a public
//! constructor, so
//! [`BlockDesc`](crate::base::block::BlockDesc),
//! [`BlockPin`](crate::base::block::BlockPin) and the pin types can
//! change without breaking blocks compiled against an older macro. The
//! macro crate is pinned to this crate's exact version, so this module
//! may change in any release.

use libhaystack::val::kind::HaystackKind;
use uuid::Uuid;

use crate::base::block::{BlockDesc, BlockPin};
use crate::blocks::{InputImpl, OutputImpl};

/// What the `BlockProps` derive reads from a block's attributes, with
/// its defaults already applied.
pub struct NativeDesc<'a> {
    /// The block name: the struct's name.
    pub name: &'a str,
    /// `#[library = "..."]`.
    pub library: &'a str,
    /// `#[dis = "..."]`.
    pub dis: &'a str,
    /// `#[category = "..."]`.
    pub category: &'a str,
    /// `#[ver = "..."]`.
    pub ver: &'a str,
    /// The struct's doc comment.
    pub doc: &'a str,
    /// The input pins, in order: name and kind.
    pub inputs: &'a [(&'a str, HaystackKind)],
    /// The output pins, in order: name and kind.
    pub outputs: &'a [(&'a str, HaystackKind)],
}

/// The description of a native block.
pub fn native_desc(desc: NativeDesc<'_>) -> BlockDesc {
    let pins = |pins: &[(&str, HaystackKind)]| {
        pins.iter()
            .map(|(name, kind)| BlockPin::new(*name, *kind))
            .collect()
    };
    BlockDesc {
        inputs: pins(desc.inputs),
        outputs: pins(desc.outputs),
        ..BlockDesc::new(desc.name, desc.library)
            .with_dis(desc.dis)
            .with_category(desc.category)
            .with_ver(desc.ver)
            .with_doc(desc.doc)
    }
}

/// The input pin `name` of block `block_id`.
pub fn input(name: &str, kind: HaystackKind, block_id: Uuid) -> InputImpl {
    InputImpl::new(name, kind, block_id)
}

/// The output pin `name` of block `block_id`.
pub fn output(name: &str, kind: HaystackKind, block_id: Uuid) -> OutputImpl {
    OutputImpl::new_named(name, kind, block_id)
}

#[cfg(test)]
mod test {
    use libhaystack::val::kind::HaystackKind;

    use super::{NativeDesc, native_desc};
    use crate::base::block::{BlockDesc, BlockPin};

    /// The derive's description of a block is the one the public
    /// builder makes from the same attributes.
    #[test]
    fn native_desc_matches_the_builder() {
        let desc = native_desc(NativeDesc {
            name: "Echo",
            library: "test",
            dis: "Echo block",
            category: "misc",
            ver: "2.1.0",
            doc: "Copies.",
            inputs: &[("in", HaystackKind::Number), ("gate", HaystackKind::Bool)],
            outputs: &[("out", HaystackKind::Number)],
        });

        let expected = BlockDesc::new("Echo", "test")
            .with_dis("Echo block")
            .with_category("misc")
            .with_ver("2.1.0")
            .with_doc("Copies.")
            .with_input("in", HaystackKind::Number)
            .with_input("gate", HaystackKind::Bool)
            .with_output("out", HaystackKind::Number);
        assert_eq!(desc, expected);
        assert_eq!(
            desc.outputs,
            vec![BlockPin::new("out", HaystackKind::Number)]
        );
    }
}
