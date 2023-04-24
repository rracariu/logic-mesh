// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use libhaystack::val::Value;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::LocalSet,
};
use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockProps, BlockState},
        engine::{
            messages::{BlockData, BlockInputData, BlockOutputData, EngineMessage},
            Engine,
        },
        link::{BaseLink, LinkState},
    },
    blocks::registry::schedule_block,
};

use super::block_pointer::BlockPropsPointer;

/// Creates a multi-producer single-consumer
/// channel that listen for Engine related messages that would control
/// the execution of the engine or will enable inspection of block states.
pub struct EngineMessaging {
    sender: Sender<EngineMessage>,
    receiver: Receiver<EngineMessage>,
}

// The concrete trait for the block properties
pub(super) trait BlockPropsType = BlockProps<Write = Sender<Value>, Read = Receiver<Value>>;

/// Creates an execution environment for Blocks to be run on.
///
/// Each block would be executed inside a local task in the engine's local context.
///
pub struct LocalSetEngine {
    /// Use to schedule task on the current thread
    local: LocalSet,
    /// Blocks registered with this engine, indexed by block id
    block_props: BTreeMap<Uuid, Rc<Cell<BlockPropsPointer>>>,
    /// Messaging used by external users to control
    /// and inspect this engines execution
    engine_messaging: EngineMessaging,
    /// External senders that would be interested in receiving messages from the engine
    notification_listeners: BTreeMap<uuid::Uuid, Sender<EngineMessage>>,
}

impl Engine for LocalSetEngine {
    type Write = Sender<Value>;
    type Read = Receiver<Value>;

    type Sender = Sender<EngineMessage>;

    fn blocks(&self) -> Vec<&dyn BlockProps<Write = Self::Write, Read = Self::Read>> {
        self.block_props
            .values()
            .filter_map(|props| {
                let props = props.get();
                props.get()
            })
            .map(|prop| unsafe { &*prop })
            .collect()
    }

    fn schedule<B: Block<Write = Self::Write, Read = Self::Read> + 'static>(
        &mut self,
        mut block: B,
    ) {
        self.block_props.insert(
            *block.id(),
            Rc::new(Cell::new(BlockPropsPointer::new(
                &mut block as &mut dyn BlockPropsType,
            ))),
        );

        let props = self
            .block_props
            .get_mut(block.id())
            .expect("Property should be present")
            .clone();

        self.local.spawn_local(async move {
            props.set(BlockPropsPointer::new(
                &mut block as &mut dyn BlockPropsType,
            ));

            loop {
                block.execute().await;

                if block.state() == BlockState::Terminate {
                    break;
                }
            }
        });
    }

    async fn run(&mut self) {
        loop {
            let local_tasks = &self.local;
            let mut engine_msg = None;

            local_tasks
                .run_until(async {
                    engine_msg = self.engine_messaging.receiver.recv().await;
                })
                .await;

            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    break;
                }

                self.dispatch_message(message).await;
            }
        }
    }

    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender: Self::Sender,
    ) -> Self::Sender {
        self.notification_listeners.insert(sender_id, sender);

        self.engine_messaging.sender.clone()
    }
}

impl LocalSetEngine {
    /// Construct
    pub fn new() -> Self {
        let (engine_sender, engine_receiver) = mpsc::channel(32);

        Self {
            local: LocalSet::new(),
            block_props: BTreeMap::default(),
            engine_messaging: EngineMessaging {
                sender: engine_sender,
                receiver: engine_receiver,
            },
            notification_listeners: BTreeMap::default(),
        }
    }

    async fn dispatch_message(&mut self, msg: EngineMessage) {
        match msg {
            EngineMessage::AddBlockReq(sender_uuid, block_name) => {
                let id = self.add_block(block_name);

                if let Some(id) = id {
                    self.reply_to_sender(sender_uuid, EngineMessage::AddBlockRes(id));
                }
            }

            EngineMessage::RemoveBlockReq(sender_uuid, block_id) => {
                let id = self.remove_block(&block_id);

                if let Some(id) = id {
                    self.reply_to_sender(sender_uuid, EngineMessage::RemoveBlockRes(id));
                }
            }

            EngineMessage::InspectBlockReq(sender_uuid, block_uuid) => {
                if let Some(block) = self.get_block_props_mut(&block_uuid) {
                    let data = BlockData {
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
                                        val: input
                                            .get_value()
                                            .as_ref()
                                            .cloned()
                                            .unwrap_or_default(),
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

                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::InspectBlockRes(sender_uuid, Some(data)),
                    );
                } else {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::InspectBlockRes(sender_uuid, None),
                    );
                }
            }

            EngineMessage::ConnectBlocksReq(sender_uuid, link_data) => {
                if (!self.block_props.contains_key(&link_data.source_block_uuid)
                    || !self.block_props.contains_key(&link_data.target_block_uuid))
                    || (link_data.source_block_uuid == link_data.target_block_uuid)
                {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::ConnectBlocksRes(sender_uuid, None),
                    );
                    return;
                }

                let tx = if let Some(target_block) =
                    self.get_block_props_mut(&link_data.target_block_uuid)
                {
                    let mut inputs = target_block.inputs_mut();
                    let input = inputs
                        .iter_mut()
                        .find(|input| input.name() == link_data.target_block_input_name);

                    if let Some(input) = input {
                        input.increment_conn();
                        Some(input.writer().clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if tx.is_none() {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::ConnectBlocksRes(sender_uuid, None),
                    );
                    return;
                }

                if let Some(source_block) = self.get_block_props_mut(&link_data.source_block_uuid) {
                    // Ignore connections to the same block and the same input.
                    if source_block.links().iter().any(|link| {
                        link.target_block_id() == &link_data.target_block_uuid
                            && link.target_input() == link_data.target_block_input_name
                    }) {
                        self.reply_to_sender(
                            sender_uuid,
                            EngineMessage::ConnectBlocksRes(sender_uuid, None),
                        );
                        return;
                    }

                    let mut link = BaseLink::<Sender<Value>>::new(
                        link_data.target_block_uuid,
                        link_data.target_block_input_name.to_string(),
                    );

                    link.tx = tx;
                    link.state = LinkState::Connected;
                    source_block
                        .outputs_mut()
                        .iter_mut()
                        .for_each(|l| l.add_link(link.clone()));

                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::ConnectBlocksRes(sender_uuid, Some(link_data)),
                    );
                }
            }

            _ => unreachable!("Invalid message"),
        }
    }

    fn reply_to_sender(&mut self, sender_uuid: Uuid, engine_message: EngineMessage) {
        for (sender_id, sender) in self.notification_listeners.iter() {
            if sender_id != &sender_uuid {
                continue;
            }

            let _ = sender.try_send(engine_message.clone());
        }
    }

    fn get_block_props_mut(
        &mut self,
        block_id: &Uuid,
    ) -> Option<&mut (dyn BlockPropsType + 'static)> {
        self.block_props.get_mut(block_id).and_then(|ptr| {
            let fat_ptr = (**ptr).get();
            fat_ptr.get().map(|ptr| unsafe { &mut *ptr })
        })
    }

    fn add_block(&mut self, block_name: String) -> Option<Uuid> {
        schedule_block(&block_name, self).ok()
    }

    fn remove_block(&mut self, block_id: &Uuid) -> Option<Uuid> {
        let res = self.get_block_props_mut(block_id).map(|block| {
            block.set_state(BlockState::Terminate);
            *block.id()
        });
        self.block_props.remove(block_id);
        res
    }
}

impl Default for LocalSetEngine {
    fn default() -> Self {
        Self::new()
    }
}
