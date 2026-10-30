// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::BTreeSet;

// External blocks only exist on `wasm32` with the `js` bindings, where
// the host supplies their executor; every other build rejects them
// with `ExternalError`.
use crate::base::error::{EngineError, ExternalError, Result, parse_block_uuid};
use libhaystack::val::Value;
use uuid::Uuid;

use crate::{
    base::{
        block::{BlockDesc, BlockImplementation},
        program::{Program, ProgramBlock},
    },
    blocks::registry::{BlockSink, eval_static_block, make_block_into},
};

pub use client::{EngineClient, Watch};
pub use graph::BlockHandle;

/// The capacity of an engine's request channel: how many requests its
/// clients can queue before sending one waits.
pub(super) const REQUEST_CHANNEL_CAP: usize = 32;

mod actor_step;
mod block_mailbox;
mod client;
mod connectors;
mod graph;
mod message_dispatch;
mod pin_changes;
pub mod single_threaded;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test_support;

#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub mod multi_threaded;

/// The unqualified name of the engine phase `S`, as an engine's
/// `Debug` output shows it.
fn phase_name<S: crate::base::engine::EngineState>() -> &'static str {
    let name = std::any::type_name::<S>();
    name.rsplit("::").next().unwrap_or(name)
}

/// Constructs the block `block` describes and hands it to `sink` — an
/// engine in either phase — returning its id.
pub(super) fn schedule_block_on_engine(
    block: &BlockDesc,
    block_id: Option<Uuid>,
    sink: &mut impl BlockSink,
) -> Result<Uuid> {
    if block.implementation == BlockImplementation::External {
        #[cfg(all(target_arch = "wasm32", feature = "js"))]
        {
            use crate::wasm::js_block::schedule_js_block;
            schedule_js_block(sink, block, block_id)
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "js")))]
        {
            Err(ExternalError::Unsupported.into())
        }
    } else {
        make_block_into(&block.name, Some(&block.library), block_id, sink)
    }
}

/// Checks, without constructing anything, that
/// [`schedule_block_on_engine`] supports the kind of block `block`
/// describes: external blocks only exist on `wasm32` with the `js`
/// bindings, where the host supplies their executor.
pub(super) fn ensure_schedulable(block: &BlockDesc) -> Result<()> {
    if cfg!(not(all(target_arch = "wasm32", feature = "js")))
        && block.implementation == BlockImplementation::External
    {
        return Err(ExternalError::Unsupported.into());
    }
    Ok(())
}

/// [`ensure_schedulable`] for [`schedule_block_on_engine_mt`].
#[cfg(feature = "multi-threaded")]
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn ensure_schedulable_mt(block: &BlockDesc) -> Result<()> {
    if block.implementation == BlockImplementation::External {
        return Err(ExternalError::UnsupportedMultiThreaded.into());
    }
    Ok(())
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
        #[cfg(all(target_arch = "wasm32", feature = "js"))]
        {
            use crate::wasm::js_block::eval_js_block;
            eval_js_block(block, inputs).await
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "js")))]
        {
            Err(ExternalError::Unsupported.into())
        }
    } else {
        eval_static_block(&block.name, Some(&block.library), inputs).await
    }
}
