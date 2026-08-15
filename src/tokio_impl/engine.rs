// Copyright (c) 2022-2023, Radu Racariu.

use crate::base::error::Result;
// External blocks only exist on `wasm32`, where the host supplies their
// executor; every other target rejects them with this error.
#[cfg(not(target_arch = "wasm32"))]
use crate::base::error::ExternalError;
use libhaystack::val::Value;
use uuid::Uuid;

use crate::{
    base::block::{BlockDesc, desc::BlockImplementation},
    blocks::registry::{eval_static_block, schedule_block, schedule_block_with_uuid},
};

use self::single_threaded::SingleThreadedEngine;

mod block_mailbox;
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
            schedule_js_block(engine, block, block_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Err(ExternalError::Unsupported.into())
        }
    } else if let Some(uuid) = block_id {
        schedule_block_with_uuid(&block.name, Some(&block.library), uuid, engine)
    } else {
        schedule_block(&block.name, Some(&block.library), engine)
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
        Err(ExternalError::UnsupportedMultiThreaded.into())
    } else if let Some(uuid) = block_id {
        schedule_block_send_with_uuid(&block.name, Some(&block.library), uuid, engine)
    } else {
        schedule_block_send(&block.name, Some(&block.library), engine)
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
            Err(ExternalError::Unsupported.into())
        }
    } else {
        eval_static_block(&block.name, Some(&block.library), inputs).await
    }
}
