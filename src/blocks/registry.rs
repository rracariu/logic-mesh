// Copyright (c) 2022-2023, IntriSemantics Corp.

use crate::base::block::{BlockDesc, BlockProps};
use crate::blocks::maths::Add;
use crate::blocks::misc::SineWave;
use lazy_static::lazy_static;
use std::collections::BTreeMap;

lazy_static! {
    pub static ref BLOCKS: BTreeMap<String, &'static BlockDesc> = {
        let mut reg = BTreeMap::new();

        let desc = Add::desc();
        reg.insert(desc.name.clone(), desc);

        let desc = SineWave::desc();
        reg.insert(desc.name.clone(), desc);

        reg
    };
}
