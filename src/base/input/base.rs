// Copyright (c) 2022-2023, Radu Racariu.

//! Base input type.

use libhaystack::val::{Value, kind::HaystackKind};
use uuid::Uuid;

use crate::base::Status;
use crate::base::link::{BaseLink, Link};
use crate::base::sealed::Sealed;

use super::InputProps;

/// The base input type.
///
/// Its fields are private: read them through [`InputProps`] and
/// [`Input`](crate::base::input::Input) (name, kind, value, status,
/// links, connectedness) and [`connection_count`](Self::connection_count),
/// and give an input a value with
/// [`InputImpl::set`](crate::blocks::InputImpl::set), which also forwards
/// it to the inputs this one links to.
#[derive(Debug, Default)]
pub struct BaseInput<Reader, Writer> {
    /// The block-unique input name.
    pub(crate) name: String,
    /// The kind of data this input can receive.
    pub(crate) kind: HaystackKind,
    /// The block id of the block this input belongs to.
    pub(crate) block_id: Uuid,
    /// The number of connections this input has.
    pub(crate) connection_count: usize,
    /// The input reader.
    pub(crate) reader: Reader,
    /// The input writer.
    pub(crate) writer: Writer,
    /// The input value.
    pub(crate) val: Option<Value>,
    /// The status of the last value received on this input. Tracks the
    /// producer's quality assertion across the watch channel.
    pub(crate) status: Status,
    /// The links to other inputs.
    pub(crate) links: Vec<BaseLink<Writer>>,
}

impl<Reader, Writer> BaseInput<Reader, Writer> {
    /// The number of links that feed this input: from outputs, or from
    /// inputs that fan out to it.
    pub fn connection_count(&self) -> usize {
        self.connection_count
    }
}

impl<Reader, Writer> Sealed for BaseInput<Reader, Writer> {}

/// Implements the [`InputProps`] trait for [`BaseInput`].
///
/// `Writer:` [`Send`] is required so the link list can return
/// `Vec<&(dyn Link + Send)>` — see the [`InputProps::links`] doc.
impl<Reader, Writer: Clone + Send> InputProps for BaseInput<Reader, Writer> {
    type Reader = Reader;
    type Writer = Writer;

    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> &HaystackKind {
        &self.kind
    }

    fn block_id(&self) -> &Uuid {
        &self.block_id
    }

    fn is_connected(&self) -> bool {
        self.connection_count > 0
    }

    fn links(&self) -> Vec<&(dyn Link + Send)> {
        self.links.iter().map(|l| l as &(dyn Link + Send)).collect()
    }

    fn add_link(&mut self, link: BaseLink<Self::Writer>) {
        self.links.push(link)
    }

    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.links.retain(|l| l.id() != link_id)
    }

    fn remove_target_block_links(&mut self, block_id: &Uuid) {
        self.links.retain(|l| l.target_block_id() != block_id)
    }

    fn remove_all_links(&mut self) {
        self.links.clear()
    }

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.reader
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }

    fn get_value(&self) -> Option<&Value> {
        self.val.as_ref()
    }

    fn increment_conn(&mut self) -> usize {
        self.connection_count += 1;
        self.connection_count
    }

    fn decrement_conn(&mut self) -> usize {
        if self.connection_count > 0 {
            self.connection_count -= 1;

            if self.connection_count == 0 {
                self.val = None;
                self.status = Status::Ok;
            }
        }

        self.connection_count
    }
}
