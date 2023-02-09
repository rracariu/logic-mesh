use libhaystack::val::kind::HaystackKind;

use super::link::{BaseLink, Link};

#[derive(Debug)]
pub struct OutDesc {
    pub name: String,
    pub kind: HaystackKind,
}

pub trait OutputProps {
    fn desc(&self) -> &OutDesc;

    fn is_connected(&self) -> bool;

    fn links(&self) -> Vec<&dyn Link>;
}

pub trait Output: OutputProps {
    type Tx: Clone;

    fn add_link(&mut self, link: BaseLink<Self::Tx>);

    fn remove_link(&mut self, link: &dyn Link);
}

#[derive(Debug)]
pub struct BaseOutput<L: Link> {
    desc: OutDesc,
    pub links: Vec<L>,
}

impl<L: Link> OutputProps for BaseOutput<L> {
    fn desc(&self) -> &OutDesc {
        &self.desc
    }

    fn is_connected(&self) -> bool {
        !self.links.is_empty()
    }

    fn links(&self) -> Vec<&dyn Link> {
        self.links.iter().map(|l| l as &dyn Link).collect()
    }
}

impl<L: Link> BaseOutput<L> {
    pub fn new_named(name: &str, kind: HaystackKind) -> Self {
        Self {
            desc: OutDesc {
                name: name.to_string(),
                kind,
            },
            links: Vec::new(),
        }
    }

    pub fn new(kind: HaystackKind) -> Self {
        Self::new_named("out", kind)
    }
}
