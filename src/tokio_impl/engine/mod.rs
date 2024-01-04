// Copyright (c) 2022-2023, Radu Racariu.

use anyhow::Result;
use uuid::Uuid;

use crate::{
    base::block::{desc::BlockImplementation, BlockDesc},
    blocks::registry::{schedule_block, schedule_block_with_uuid},
};

use self::single_threaded::SingleThreadedEngine;

mod block_pointer;
mod message_dispatch;
pub mod single_threaded;

pub(super) fn schedule_block_on_engine(
    block: &BlockDesc,
    block_id: Option<Uuid>,
    engine: &mut SingleThreadedEngine,
) -> Result<Uuid> {
    if block.implementation == BlockImplementation::External {
        #[cfg(target_arch = "wasm32")]
        {
            use crate::wasm::js_block::schedule_js_block;
            schedule_js_block(engine, &block, block_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use anyhow::anyhow;
            Err(anyhow!("External blocks not supported on this platform"))
        }
    } else if let Some(uuid) = block_id {
        schedule_block_with_uuid(&block.name, uuid, engine)
    } else {
        schedule_block(&block.name, engine)
    }
}
