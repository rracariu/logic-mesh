// Copyright (c) 2022-2023, IntriSemantics Corp.

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::link::Link;

use super::{props::OutDesc, OutputProps};
#[derive(Debug)]
pub struct BaseOutput<L: Link> {
    desc: OutDesc,
    pub value: Value,
    pub links: Vec<L>,
    pub block_id: Uuid,
}

impl<L: Link> OutputProps for BaseOutput<L> {
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

    fn links(&self) -> Vec<&dyn Link> {
        self.links.iter().map(|l| l as &dyn Link).collect()
    }
}

impl<L: Link> BaseOutput<L> {
    pub fn new_named(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
        Self {
            desc: OutDesc {
                name: name.to_string(),
                kind,
            },
            value: Value::default(),
            links: Vec::new(),
            block_id,
        }
    }

    pub fn new(kind: HaystackKind, block_id: Uuid) -> Self {
        Self::new_named("out", kind, block_id)
    }
}
