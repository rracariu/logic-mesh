// Copyright (c) 2022-2023, Radu Racariu.

//! Base output properties.

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::link::Link;
use crate::base::sealed::Sealed;

/// The description of an output pin.
///
/// `#[non_exhaustive]`: build one with [`OutDesc::new`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct OutDesc {
    /// The output's name.
    pub name: String,
    /// The output's haystack kind.
    pub kind: HaystackKind,
}

impl OutDesc {
    /// The description of an output named `name` that produces values
    /// of `kind`.
    pub fn new(name: impl Into<String>, kind: HaystackKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }
}

/// Properties of a block output pin.
///
/// # Sealed
///
/// Only [`BaseOutput`](super::BaseOutput) implements this trait, and no
/// other crate can, so methods can be added to it, and to its subtrait
/// [`Output`](super::Output), in a minor release. A block declares its
/// outputs as [`OutputImpl`](crate::blocks::OutputImpl) fields:
///
/// ```compile_fail,E0277
/// use logic_mesh::base::link::Link;
/// use logic_mesh::base::output::{OutputProps, props::OutDesc};
/// use logic_mesh::{Uuid, Value};
///
/// struct MyOutput {
///     desc: OutDesc,
///     block_id: Uuid,
///     value: Value,
/// }
///
/// impl OutputProps for MyOutput {
///     fn desc(&self) -> &OutDesc {
///         &self.desc
///     }
///     fn block_id(&self) -> &Uuid {
///         &self.block_id
///     }
///     fn is_connected(&self) -> bool {
///         false
///     }
///     fn links(&self) -> Vec<&(dyn Link + Send)> {
///         Vec::new()
///     }
///     fn remove_link_by_id(&mut self, _link_id: &Uuid) {}
///     fn remove_target_block_links(&mut self, _block_id: &Uuid) {}
///     fn remove_all_links(&mut self) {}
///     fn value(&self) -> &Value {
///         &self.value
///     }
/// }
/// ```
pub trait OutputProps: Sealed {
    /// Returns the output's description.
    fn desc(&self) -> &OutDesc;

    /// Returns the output's name.
    fn name(&self) -> &str {
        &self.desc().name
    }

    /// Returns the block id of the block this output belongs to.
    fn block_id(&self) -> &Uuid;

    /// Returns `true` if this output is connected to at least one input.
    fn is_connected(&self) -> bool;

    /// Returns links from this output. The trait objects carry a
    /// `+` [`Send`] bound so they can be held across `.await` points in the
    /// MT actor future.
    fn links(&self) -> Vec<&(dyn Link + Send)>;

    /// Removes a link from this output.
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Removes a link by id from this output.
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Removes all links to a specific block from this output.
    fn remove_target_block_links(&mut self, block_id: &Uuid);

    /// Removes all links from this output.
    fn remove_all_links(&mut self);

    /// Returns this output's value.
    fn value(&self) -> &Value;
}
