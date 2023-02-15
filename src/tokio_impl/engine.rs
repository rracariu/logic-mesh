// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

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
    engine_messages::{BlockData, BlockInputData, BlockMessage, BlockOutputData},
};
use crate::blocks::{
    maths::Add,
    misc::{Random, SineWave},
};

use crate::base::engine_messages::EngineMessage;

/// Creates a multi-producer single-consumer
/// channel that listen for Engine related messages that would control
/// the execution of the engine or will enable inspection of block states.
pub struct EngineMessaging<T> {
    sender: Sender<T>,
    receiver: Receiver<T>,
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
    blocks: BTreeMap<String, BlockDesc>,
    /// Messaging used by external users to control
    /// and inspect this engines execution
    engine_messaging: EngineMessaging<EngineMessage>,
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
    pub fn schedule<B: Block + 'static>(&mut self, mut block: B) {
        self.blocks
            .insert(block.id().to_string(), B::desc().clone());

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
                        BlockMessage::InspectBlock(sender_uuid, uuid) => {
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
                                                    .unwrap_or_default()
                                                    .clone(),
                                            },
                                        )
                                    })
                                    .collect(),
                                output: BlockOutputData {
                                    kind: block.output().desc().kind.to_string(),
                                    val: block.output().value().clone(),
                                },
                            };

                            if uuid == block.id() {
                                let _ =
                                    sender.try_send(EngineMessage::BlockData(*sender_uuid, data));
                            }
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
                    self.reply_to_sender(sender_uuid, EngineMessage::BlockAdded(id.clone()));
                }
            }

            EngineMessage::InspectBlock(sender_uuid, block_uuid) => {
                let _ = self
                    .block_messaging
                    .notifications
                    .send(BlockMessage::InspectBlock(sender_uuid, block_uuid));
            }

            EngineMessage::BlockData(sender_uuid, data) => {
                self.reply_to_sender(sender_uuid, EngineMessage::BlockData(sender_uuid, data));
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
