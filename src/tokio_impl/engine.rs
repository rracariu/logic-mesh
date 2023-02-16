// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        watch,
    },
    task::LocalSet,
};
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps},
    engine_messages::{BlockData, BlockInputData, BlockMessage, BlockOutputData, LinkData},
    link::{BaseLink, LinkState},
};
use crate::blocks::{
    maths::Add,
    misc::{Random, SineWave},
};

use crate::base::engine_messages::EngineMessage;

/// Creates a multi-producer single-consumer
/// channel that listen for Engine related messages that would control
/// the execution of the engine or will enable inspection of block states.
pub struct EngineMessaging {
    sender: Sender<EngineMessage>,
    receiver: Receiver<EngineMessage>,
}

/// Intra engine messaging channels.
/// These are used by the engine to communicated to each block task.
///
/// Each block task would listen to the notifications that would be send via the notification
/// channel, those block tasks could then reply to those notifications via the engine sender channel.
/// The engine would then listen to the sender channel to receive messages from multiple blocks.
struct BlockMessaging {
    /// Channel used be engine to broadcast messages to multiple block tasks
    notifications: watch::Sender<BlockMessage>,
}

/// Creates an execution environment for Blocks to be run on.
///
/// Each block would be executed inside a local task in the engine's local context.
///
pub struct Engine {
    /// Use to schedule task on the current thread
    local: LocalSet,
    /// Blocks registered with this engine, indexed by block id
    blocks: BTreeMap<Uuid, &'static BlockDesc>,
    /// Messaging used by external users to control
    /// and inspect this engines execution
    engine_messaging: EngineMessaging,
    /// External senders that would be interested in receiving messages from the engine
    notification_listeners: BTreeMap<uuid::Uuid, Sender<EngineMessage>>,
    /// Internal messaging for used to communicate
    /// with the block tasks
    block_messaging: BlockMessaging,
}

impl Engine {
    /// Construct
    pub fn new() -> Self {
        let (engine_sender, engine_receiver) = mpsc::channel(32);
        let (block_sender, _) = watch::channel::<BlockMessage>(BlockMessage::Nop);

        Self {
            local: LocalSet::new(),
            blocks: BTreeMap::default(),
            engine_messaging: EngineMessaging {
                sender: engine_sender,
                receiver: engine_receiver,
            },
            notification_listeners: BTreeMap::default(),
            block_messaging: BlockMessaging {
                notifications: block_sender,
            },
        }
    }

    /// Schedule a block to be executed by this engine
    pub fn schedule<B: Block<Tx = Sender<Value>, Rx = Receiver<Value>> + 'static>(
        &mut self,
        mut block: B,
    ) {
        self.blocks.insert(*block.id(), B::desc());

        let mut receiver = self.block_messaging.notifications.subscribe();
        let sender = self.engine_messaging.sender.clone();

        self.local.spawn_local(async move {
            loop {
                block.execute().await;

                if receiver.has_changed().is_ok() {
                    if receiver.changed().await.is_err() {
                        continue;
                    }

                    let msg = &*receiver.borrow();

                    match msg {
                        BlockMessage::InspectBlock(sender_uuid, block_uuid) => {
                            if block_uuid != block.id() {
                                continue;
                            }

                            let data = BlockData {
                                id: block.id().to_string(),
                                name: B::desc().name.clone(),
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
                                output: BlockOutputData {
                                    kind: block.output().desc().kind.to_string(),
                                    val: block.output().value().clone(),
                                },
                            };

                            let _ =
                                sender.try_send(EngineMessage::FoundBlockData(*sender_uuid, data));
                        }

                        BlockMessage::GetInputWriter(sender_uuid, link_data) => {
                            let LinkData {
                                source_block_uuid: _,
                                target_block_uuid,
                                target_block_input_name,
                            } = link_data;

                            if target_block_uuid != block.id() {
                                continue;
                            }

                            let mut inputs = block.inputs_mut();
                            let input = inputs
                                .iter_mut()
                                .find(|input| input.name() == target_block_input_name);

                            if let Some(input) = input {
                                input.increment_conn();
                                let tx = input.writer().clone();
                                let _ = sender.try_send(EngineMessage::FoundBlockInputWriter(
                                    *sender_uuid,
                                    link_data.clone(),
                                    tx,
                                ));
                            }
                        }

                        BlockMessage::ConnectBlocks(sender_uuid, link_data, writer) => {
                            let LinkData {
                                source_block_uuid,
                                target_block_uuid,
                                target_block_input_name,
                            } = link_data;

                            if source_block_uuid != block.id() {
                                continue;
                            }

                            // Ignore connections to the same block and the same input.
                            if block.links().iter().any(|link| {
                                link.target_block_id() == target_block_uuid
                                    && link.target_input() == target_block_input_name
                            }) {
                                continue;
                            }

                            let mut link = BaseLink::<Sender<Value>>::new(
                                *target_block_uuid,
                                target_block_input_name.to_string(),
                            );

                            link.tx = Some(writer.clone());
                            link.state = LinkState::Connected;
                            block.output_mut().add_link(link);

                            let _ = sender.try_send(EngineMessage::LinkCreated(
                                *sender_uuid,
                                Some(link_data.clone()),
                            ));
                        }
                        _ => unreachable!("Invalid block message."),
                    };
                }
            }
        });
    }

    /// Get a handle to this engines messaging system so external
    /// systems can communicate with this engine.
    ///
    /// # Arguments
    /// - sender_id The sender unique id.
    /// - sender The chanel to send notifications from the engine.
    ///
    /// # Returns
    /// A sender chanel that is used to send messages to the engine.
    ///
    pub fn message_handles(
        &mut self,
        sender_id: uuid::Uuid,
        sender: Sender<EngineMessage>,
    ) -> Sender<EngineMessage> {
        self.notification_listeners.insert(sender_id, sender);

        self.engine_messaging.sender.clone()
    }

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    pub async fn run(&mut self) {
        loop {
            let local_tasks = &self.local;
            let mut engine_msg = None;

            local_tasks
                .run_until(async {
                    engine_msg = self.engine_messaging.receiver.recv().await;
                })
                .await;

            if let Some(message) = engine_msg {
                self.dispatch_message(message).await;
            }
        }
    }

    async fn dispatch_message(&mut self, msg: EngineMessage) {
        match msg {
            EngineMessage::AddBlock(sender_uuid, block_name) => {
                let id = self.add_block(block_name);

                if let Some(id) = id {
                    self.reply_to_sender(sender_uuid, EngineMessage::BlockAdded(id));
                }
            }

            EngineMessage::InspectBlock(sender_uuid, block_uuid) => {
                let _ = self
                    .block_messaging
                    .notifications
                    .send(BlockMessage::InspectBlock(sender_uuid, block_uuid));
            }

            EngineMessage::FoundBlockData(sender_uuid, data) => {
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::FoundBlockData(sender_uuid, data),
                );
            }

            EngineMessage::ConnectBlocks(sender_uuid, link_data) => {
                if (!self.blocks.contains_key(&link_data.source_block_uuid)
                    || !self.blocks.contains_key(&link_data.target_block_uuid))
                    || (link_data.source_block_uuid == link_data.target_block_uuid)
                {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::LinkCreated(sender_uuid, None),
                    );
                    return;
                }

                let _ = self
                    .block_messaging
                    .notifications
                    .send(BlockMessage::GetInputWriter(sender_uuid, link_data));
            }

            EngineMessage::FoundBlockInputWriter(sender_uuid, link_data, writer) => {
                if !self.blocks.contains_key(&link_data.source_block_uuid)
                    || !self.blocks.contains_key(&link_data.target_block_uuid)
                {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::LinkCreated(sender_uuid, None),
                    );
                    return;
                }

                let _ = self
                    .block_messaging
                    .notifications
                    .send(BlockMessage::ConnectBlocks(sender_uuid, link_data, writer));
            }

            EngineMessage::LinkCreated(sender_uuid, link_data) => {
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::LinkCreated(sender_uuid, link_data),
                );
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

    fn add_block(&mut self, block_name: String) -> Option<Uuid> {
        match block_name.as_str() {
            "Add" => {
                let block = Add::new(&block_name);
                let id = *block.id();
                self.schedule(block);
                Some(id)
            }
            "Random" => {
                let block = Random::new(&block_name);
                let id = *block.id();
                self.schedule(block);
                Some(id)
            }
            "SineWave" => {
                let block = SineWave::new(&block_name);
                let id = *block.id();
                self.schedule(block);
                Some(id)
            }

            _ => None,
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
