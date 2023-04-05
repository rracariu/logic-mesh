// Copyright (c) 2022-2023, IntriSemantics Corp.

use crate::base::block::{BlockDesc, BlockDescAccess};
use crate::blocks::maths::Add;
use crate::blocks::misc::{Random, SineWave};
use lazy_static::lazy_static;
use std::collections::BTreeMap;

lazy_static! {
    /// The list of all registered blocks
    pub static ref BLOCKS: BTreeMap<String, &'static BlockDesc> = {
        let mut reg = BTreeMap::new();

        let desc = Add::desc();
        reg.insert(desc.kind.clone(), desc);

        let desc = Random::desc();
        reg.insert(desc.kind.clone(), desc);

        let desc = SineWave::desc();
        reg.insert(desc.kind.clone(), desc);

        reg
    };
}
