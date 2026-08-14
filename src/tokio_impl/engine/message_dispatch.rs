// Copyright (c) 2022-2026, Radu Racariu.

//! Engine-message dispatcher.
//!
//! Each `EngineMessage` arriving on the engine's external channel is
//! translated into one (or more) per-block mailbox round-trips against the
//! actor tasks owned by `SingleThreadedEngine`.

use crate::base::engine::messages::EngineMessage;

use crate::base::error::{RegistryError, parse_block_uuid};
use crate::blocks::registry::{CORE_LIB, get_block};
use crate::single_threaded::Messages;
use crate::single_threaded::SingleThreadedEngine;
use uuid::Uuid;

use super::eval_block;

pub(super) async fn dispatch_message(engine: &mut SingleThreadedEngine, msg: Messages) {
    match msg {
        EngineMessage::AddBlockReq(sender_uuid, block_name, block_uuid, lib) => {
            log::debug!(
                "Adding block: {}::{}",
                lib.clone().unwrap_or("core".into()),
                block_name,
            );

            let block_id = match block_uuid.as_deref().map(parse_block_uuid).transpose() {
                Ok(id) => id,
                Err(err) => {
                    return reply_to_sender(
                        engine,
                        sender_uuid,
                        EngineMessage::AddBlockRes(Err(err.to_string())),
                    );
                }
            };

            let block_id = engine
                .add_block(block_name, block_id, lib.as_deref())
                .await
                .map_err(|err| err.to_string());

            reply_to_sender(engine, sender_uuid, EngineMessage::AddBlockRes(block_id));
        }

        EngineMessage::RemoveBlockReq(sender_uuid, block_id) => {
            log::debug!("Removing block: {:?}", block_id);

            let block_id = engine
                .remove_block(&block_id)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(engine, sender_uuid, EngineMessage::RemoveBlockRes(block_id));
        }

        EngineMessage::InspectBlockReq(sender_uuid, block_uuid) => {
            let response = engine
                .inspect_block(&block_uuid)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::InspectBlockRes(response),
            );
        }

        EngineMessage::EvaluateBlockReq(sender_uuid, name, inputs, lib) => {
            let Some(block) = get_block(name.as_str(), lib.as_deref()) else {
                let err = RegistryError::BlockNotFound {
                    library: lib.as_deref().unwrap_or(CORE_LIB).to_string(),
                    name,
                };
                return reply_to_sender(
                    engine,
                    sender_uuid,
                    EngineMessage::EvaluateBlockRes(Err(err.to_string())),
                );
            };

            let response = eval_block(&block.desc, inputs).await;

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::EvaluateBlockRes(response.map_err(|err| err.to_string())),
            );
        }

        EngineMessage::WriteBlockOutputReq(sender_uuid, block_uuid, output_name, value) => {
            let response = engine
                .write_output(&block_uuid, output_name, value)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::WriteBlockOutputRes(response),
            );
        }

        EngineMessage::WriteBlockInputReq(sender_uuid, block_uuid, input_name, value) => {
            let response = engine
                .write_input(&block_uuid, input_name, value)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::WriteBlockInputRes(response),
            );
        }

        EngineMessage::WatchBlockSubReq(sender_uuid, sender) => {
            engine.watchers.borrow_mut().insert(sender_uuid, sender);

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::WatchBlockSubRes(Ok(sender_uuid)),
            );
        }

        EngineMessage::WatchBlockUnsubReq(sender_uuid) => {
            engine.watchers.borrow_mut().remove(&sender_uuid);

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::WatchBlockUnsubRes(Ok(sender_uuid)),
            );
        }

        EngineMessage::GetCurrentProgramReq(sender_uuid) => {
            log::debug!("GetCurrentProgramReq");

            let program = engine.save_program().await.map_err(|err| err.to_string());

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::GetCurrentProgramRes(program),
            );
        }

        EngineMessage::LoadProgramReq(sender_uuid, program) => {
            log::debug!(
                "LoadProgramReq: {} blocks, {} links",
                program.blocks.len(),
                program.links.len()
            );

            let res = engine
                .load_program(program)
                .await
                .map_err(|err| err.to_string());

            reply_to_sender(engine, sender_uuid, EngineMessage::LoadProgramRes(res));
        }

        EngineMessage::ConnectBlocksReq(sender_uuid, link_data) => {
            log::debug!("ConnectBlocksReq: {:?}", link_data);

            let res = engine
                .connect_blocks(&link_data)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(engine, sender_uuid, EngineMessage::ConnectBlocksRes(res));
        }

        EngineMessage::RemoveLinkReq(sender_uuid, link_id) => {
            log::debug!("RemoveLinkReq: {:?}", link_id);

            let res = engine
                .disconnect_link_by_id(&link_id)
                .await
                .map_err(|err| err.to_string());
            reply_to_sender(engine, sender_uuid, EngineMessage::RemoveLinkRes(res));
        }

        _ => unreachable!("Invalid message"),
    }
}

fn reply_to_sender(engine: &mut SingleThreadedEngine, sender_uuid: Uuid, engine_message: Messages) {
    for (sender_id, sender) in engine.reply_senders.iter() {
        if sender_id != &sender_uuid {
            continue;
        }

        let _ = sender.try_send(engine_message.clone());
    }
}
