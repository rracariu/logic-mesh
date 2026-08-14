// Copyright (c) 2022-2023, Radu Racariu.

//! Block properties trait.

use uuid::Uuid;

use crate::base::{input::Input, link::Link, output::Output};

use super::{BlockState, desc::BlockDesc};

/// Trait-alias shorthand for "an [`Input`] with reader/writer types
/// pinned to `R` and `W`". Lets `BlockProps` return-position types stay
/// readable instead of repeating
/// `Input<Reader = Self::Reader, Writer = Self::Writer>` everywhere.
/// Auto-implemented for any matching `Input`; `?Sized` so it works in
/// trait-object positions.
pub trait BlockInput<R, W: Clone>: Input<Reader = R, Writer = W> {}
impl<R, W: Clone, T: ?Sized + Input<Reader = R, Writer = W>> BlockInput<R, W> for T {}

/// Trait-alias shorthand for "an [`Output`] with writer type pinned to
/// `W`". Same role as [`BlockInput`] — keeps signatures terse.
pub trait BlockOutput<W: Clone>: Output<Writer = W> {}
impl<W: Clone, T: ?Sized + Output<Writer = W>> BlockOutput<W> for T {}

/// Block properties that are common to all blocks.
///
/// Trait-object returns carry a `+ Send` bound so they can be held
/// across `.await` points in actor futures running on tokio's
/// multi-threaded runtime (see [`crate::tokio_impl::engine::multi_threaded`]).
/// Concrete impls in this crate (`BaseInput`, `BaseOutput`, the
/// mocks) are already `Send`, so the bound is satisfied transparently.
pub trait BlockProps {
    /// The block's read type, used to read from the block's inputs.
    type Reader;
    /// The block's write type, used to write to the block's outputs.
    type Writer: Clone;

    /// Returns the block's unique id.
    fn id(&self) -> &Uuid;

    /// Returns the block's name.
    fn name(&self) -> &str;

    /// Returns the block's static description.
    fn desc(&self) -> &BlockDesc;

    /// Returns the block's state.
    fn state(&self) -> BlockState;

    /// Sets the block's state.
    fn set_state(&mut self, state: BlockState) -> BlockState;

    /// Returns all the block inputs.
    fn inputs(&self) -> Vec<&(dyn BlockInput<Self::Reader, Self::Writer> + Send)>;

    /// Returns a block input by name.
    fn get_input(
        &self,
        name: &str,
    ) -> Option<&(dyn BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs().iter().find(|i| i.name() == name).cloned()
    }

    /// Returns a mutable block input by name.
    fn get_input_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut (dyn BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.inputs_mut().into_iter().find(|i| i.name() == name)
    }

    /// Returns all the block inputs as mutable references.
    fn inputs_mut(&mut self) -> Vec<&mut (dyn BlockInput<Self::Reader, Self::Writer> + Send)>;

    /// Returns all the block outputs.
    fn outputs(&self) -> Vec<&(dyn BlockOutput<Self::Writer> + Send)>;

    /// Returns a block output by name.
    fn get_output(&self, name: &str) -> Option<&(dyn BlockOutput<Self::Writer> + Send)> {
        self.outputs().iter().find(|i| i.name() == name).cloned()
    }

    /// Returns a mutable block output by name.
    fn get_output_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut (dyn BlockOutput<Self::Writer> + Send)> {
        self.outputs_mut().into_iter().find(|i| i.name() == name)
    }

    /// Returns mutable references to the block's outputs.
    fn outputs_mut(&mut self) -> Vec<&mut (dyn BlockOutput<Self::Writer> + Send)>;

    /// Returns all the links this block has.
    fn links(&self) -> Vec<(&str, Vec<&(dyn crate::base::link::Link + Send)>)>;

    /// Removes a link from the link collection.
    fn remove_link(&mut self, link: &dyn Link) {
        self.remove_link_by_id(link.id())
    }

    /// Removes a link by its id from the link collection.
    fn remove_link_by_id(&mut self, link_id: &Uuid);

    /// Removes all links from this block.
    fn remove_all_links(&mut self);
}
