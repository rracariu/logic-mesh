// Copyright (c) 2022-2023, Radu Racariu.

use anyhow::Result;
use libhaystack::val::Value;
use uuid::Uuid;

use crate::{
    base::block::{BlockDesc, desc::BlockImplementation},
    blocks::registry::{eval_static_block, schedule_block, schedule_block_with_uuid},
};

use self::single_threaded::SingleThreadedEngine;

mod message_dispatch;
pub mod single_threaded;

#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub mod multi_threaded;

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

#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn schedule_block_on_engine_mt(
    block: &BlockDesc,
    block_id: Option<Uuid>,
    engine: &mut multi_threaded::MultiThreadedEngine,
) -> Result<Uuid> {
    use crate::blocks::registry::{schedule_block_send, schedule_block_send_with_uuid};

    if block.implementation == BlockImplementation::External {
        use anyhow::anyhow;
        Err(anyhow!(
            "External blocks not supported in multi-threaded mode"
        ))
    } else if let Some(uuid) = block_id {
        schedule_block_send_with_uuid(&block.name, uuid, engine)
    } else {
        schedule_block_send(&block.name, engine)
    }
}

pub(super) async fn eval_block(block: &BlockDesc, inputs: Vec<Value>) -> Result<Vec<Value>> {
    if block.implementation == BlockImplementation::External {
        #[cfg(target_arch = "wasm32")]
        {
            use crate::wasm::js_block::eval_js_block;
            eval_js_block(block, inputs).await
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            use anyhow::anyhow;
            Err(anyhow!("External blocks not supported on this platform"))
        }
    } else {
        eval_static_block(&block.name, inputs).await
    }
}
