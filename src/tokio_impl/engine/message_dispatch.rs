// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::block::connect::disconnect_link;
use crate::base::engine::messages::BlockDefinition;
use crate::base::engine::messages::BlockInputData;
use crate::base::engine::messages::BlockOutputData;
use crate::base::engine::messages::EngineMessage;

use crate::blocks::registry::get_block;
use crate::single_threaded::Messages;
use crate::single_threaded::SingleThreadedEngine;
use libhaystack::val::Value;
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

            let block_id = if let Some(uuid) = block_uuid {
                match Uuid::parse_str(&uuid) {
                    Ok(uuid) => Some(uuid),
                    Err(_) => {
                        return reply_to_sender(
                            engine,
                            sender_uuid,
                            EngineMessage::AddBlockRes(Err("Invalid UUID".into())),
                        )
                    }
                }
            } else {
                None
            };

            let block_id = engine
                .add_block(block_name, block_id, lib)
                .map_err(|err| err.to_string());

            reply_to_sender(engine, sender_uuid, EngineMessage::AddBlockRes(block_id));
        }

        EngineMessage::RemoveBlockReq(sender_uuid, block_id) => {
            log::debug!("Removing block: {:?}", block_id);

            let block_id = engine
                .remove_block(&block_id)
                .map_err(|err| err.to_string());
            reply_to_sender(engine, sender_uuid, EngineMessage::RemoveBlockRes(block_id));
        }

        EngineMessage::InspectBlockReq(sender_uuid, block_uuid) => {
            match engine.get_block_props_mut(&block_uuid) {
                Some(block) => {
                    let data = BlockDefinition {
                        id: block.id().to_string(),
                        name: block.name().to_string(),
                        library: block.desc().library.clone(),
                        inputs: block
                            .inputs()
                            .iter()
                            .map(|input| {
                                (
                                    input.name().to_string(),
                                    BlockInputData {
                                        kind: input.kind().to_string(),
                                        val: input.get_value().cloned().unwrap_or_default(),
                                    },
                                )
                            })
                            .collect(),
                        outputs: block
                            .outputs()
                            .iter()
                            .map(|output| {
                                (
                                    output.desc().name.to_string(),
                                    BlockOutputData {
                                        kind: output.desc().kind.to_string(),
                                        val: output.value().clone(),
                                    },
                                )
                            })
                            .collect(),
                    };

                    reply_to_sender(
                        engine,
                        sender_uuid,
                        EngineMessage::InspectBlockRes(Ok(data)),
                    );
                }
                None => {
                    reply_to_sender(
                        engine,
                        sender_uuid,
                        EngineMessage::InspectBlockRes(Err("Block not found".into())),
                    );
                }
            }
        }

        EngineMessage::EvaluateBlockReq(sender_uuid, name, inputs, lib) => {
            let Some(block) = get_block(name.as_str(), lib) else {
                return reply_to_sender(
                    engine,
                    sender_uuid,
                    EngineMessage::EvaluateBlockRes(Err("Block not found".into())),
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
            let response: Result<Value, String>;

            match engine.get_block_props_mut(&block_uuid) {
                Some(block) => {
                    if let Some(output) = block.get_output_mut(&output_name) {
                        let prev = output.value().clone();
                        output.set(value);

                        response = Ok(prev);
                    } else {
                        response = Err("Output not found".to_string());
                    }
                }
                None => {
                    response = Err("Block not found".to_string());
                }
            }

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::WriteBlockOutputRes(response),
            );
        }

        EngineMessage::WriteBlockInputReq(sender_uuid, block_uuid, input_name, value) => {
            let response: Result<Option<Value>, String>;

            match engine.get_block_props_mut(&block_uuid) {
                Some(block) => {
                    if let Some(input) = block.get_input_mut(&input_name) {
                        let prev = input.get_value().cloned();

                        input.set_value(value);

                        response = Ok(prev);
                    } else {
                        response = Err("Input not found".to_string());
                    }
                }
                None => {
                    response = Err("Block not found".to_string());
                }
            }

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

            let program = engine.save_blocks_and_links();

            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::GetCurrentProgramRes(program.map_err(|err| err.to_string())),
            );
        }

        EngineMessage::ConnectBlocksReq(sender_uuid, link_data) => {
            log::debug!("ConnectBlocksReq: {:?}", link_data);

            let res = engine.connect_blocks(&link_data);
            reply_to_sender(
                engine,
                sender_uuid,
                EngineMessage::ConnectBlocksRes(res.map_err(|err| err.to_string())),
            );
        }

        EngineMessage::RemoveLinkReq(sender_uuid, link_id) => {
            log::debug!("RemoveLinkReq: {:?}", link_id);

            let res = engine.blocks_iter_mut().any(|block| {
                disconnect_link(block, &link_id, |id, name| {
                    engine.decrement_refresh_block_input(id, name)
                })
            });

            reply_to_sender(engine, sender_uuid, EngineMessage::RemoveLinkRes(Ok(res)));
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
