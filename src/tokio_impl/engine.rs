// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::BTreeSet;

use crate::base::error::{EngineError, Result, parse_block_uuid};
// External blocks only exist on `wasm32`, where the host supplies their
// executor; every other target rejects them with this error.
#[cfg(not(target_arch = "wasm32"))]
use crate::base::error::ExternalError;
use libhaystack::val::Value;
use uuid::Uuid;

use crate::{
    base::{
        block::{BlockDesc, desc::BlockImplementation},
        program::{Program, data::ProgramBlock},
    },
    blocks::registry::{BlockSink, eval_static_block, make_block_into},
};

mod actor_step;
mod block_mailbox;
mod connectors;
mod message_dispatch;
pub mod single_threaded;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test_support;

#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub mod multi_threaded;

/// Constructs the block `block` describes and hands it to `sink` — an
/// engine in either phase — returning its id.
pub(super) fn schedule_block_on_engine(
    block: &BlockDesc,
    block_id: Option<Uuid>,
    sink: &mut impl BlockSink,
) -> Result<Uuid> {
    if block.implementation == BlockImplementation::External {
        #[cfg(target_arch = "wasm32")]
        {
            use crate::wasm::js_block::schedule_js_block;
            schedule_js_block(sink, block, block_id)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Err(ExternalError::Unsupported.into())
        }
    } else {
        make_block_into(&block.name, Some(&block.library), block_id, sink)
    }
}

/// [`schedule_block_on_engine`] for the multi-threaded engine, which
/// reports external blocks with its own error.
#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn schedule_block_on_engine_mt(
    block: &BlockDesc,
    block_id: Option<Uuid>,
    sink: &mut impl BlockSink,
) -> Result<Uuid> {
    if block.implementation == BlockImplementation::External {
        Err(ExternalError::UnsupportedMultiThreaded.into())
    } else {
        make_block_into(&block.name, Some(&block.library), block_id, sink)
    }
}

/// Parses every block id of `program`, rejecting one that
/// `is_scheduled` already or that the program names twice — two keys can
/// spell the same UUID differently.
///
/// Engines run this before scheduling any of the program's blocks, so a
/// program rejected here leaves the engine untouched.
pub(super) fn program_block_ids(
    program: &Program,
    is_scheduled: impl Fn(&Uuid) -> bool,
) -> Result<Vec<(Uuid, &ProgramBlock)>, EngineError> {
    let mut seen = BTreeSet::new();
    program
        .blocks
        .iter()
        .map(|(uuid_str, block)| {
            let id = parse_block_uuid(uuid_str)?;
            if is_scheduled(&id) || !seen.insert(id) {
                return Err(EngineError::DuplicateBlockId { id });
            }
            Ok((id, block))
        })
        .collect()
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
