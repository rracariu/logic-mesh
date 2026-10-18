// Copyright (c) 2022-2023, Radu Racariu.

//! Base output type.

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::Status;
use crate::base::link::Link;
use crate::base::sealed::Sealed;

use super::{OutputProps, props::OutDesc};

/// The base implementation of an output pin.
///
/// Its fields are private: read them through [`OutputProps`] and
/// [`effective_status`](Self::effective_status), and give the output a
/// value with [`Output::set`](crate::base::output::Output::set), which
/// also sends it on the output's links.
#[derive(Debug)]
pub struct BaseOutput<L: Link> {
    desc: OutDesc,
    /// Current output value.
    pub(crate) value: Value,
    /// Downstream links attached to this output.
    pub(crate) links: Vec<L>,
    /// UUID of the block that owns this output.
    pub(crate) block_id: Uuid,
    /// Status to pair with this output's value when it's emitted on the
    /// watch channel. Managed by the actor task: set to
    /// [`Status::Fault`] while the owning block is in
    /// [`BlockState::Fault`](crate::base::block::BlockState::Fault),
    /// restored to [`Status::Ok`] on
    /// recovery.
    pub(crate) effective_status: Status,
}

impl<L: Link> Sealed for BaseOutput<L> {}

/// The implementation of the OutputProps trait.
///
/// `L:` [`Send`] is required so the link list can return
/// `Vec<&(dyn Link + Send)>` — see the [`OutputProps::links`] doc.
impl<L: Link + Send> OutputProps for BaseOutput<L> {
    fn desc(&self) -> &OutDesc {
        &self.desc
    }

    fn block_id(&self) -> &Uuid {
        &self.block_id
    }

    fn value(&self) -> &Value {
        &self.value
    }

    fn is_connected(&self) -> bool {
        !self.links.is_empty()
    }

    fn links(&self) -> Vec<&(dyn Link + Send)> {
        self.links
            .iter()
            .map(|link| link as &(dyn Link + Send))
            .collect()
    }

    /// Removes a link by id from this output.
    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.links.retain(|link| link.id() != link_id);
    }

    fn remove_target_block_links(&mut self, block_id: &Uuid) {
        self.links.retain(|link| link.target_block_id() != block_id);
    }

    fn remove_all_links(&mut self) {
        self.links.clear();
    }
}

impl<L: Link> BaseOutput<L> {
    /// Creates a new output pin with the given `name` and `kind`.
    pub fn new_named(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
        Self {
            desc: OutDesc::new(name, kind),
            value: Value::default(),
            links: Vec::new(),
            block_id,
            effective_status: Status::Ok,
        }
    }

    /// Creates a new output pin with the default name `"out"`.
    pub fn new(kind: HaystackKind, block_id: Uuid) -> Self {
        Self::new_named("out", kind, block_id)
    }

    /// The status this output sends its value with: [`Status::Fault`]
    /// while its block is faulted, [`Status::Ok`] otherwise. The engine
    /// manages it through
    /// [`Output::emit_status`](crate::base::output::Output::emit_status).
    pub fn effective_status(&self) -> Status {
        self.effective_status
    }
}
