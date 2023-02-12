// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        watch,
    },
    task::LocalSet,
};

use crate::base::block::{Block, BlockDesc};

/// Creates a multi-producer single-consumer
/// channel that listen for Engine related messages that would control
/// the execution of the engine or will enable inspection of block states.
pub struct EngineMessaging {
    sender: Sender<String>,
    receiver: Receiver<String>,
}

/// Intra engine messaging channels.
/// These are used by the engine to communicated to each block task.
///
/// Each block task would listen to the notifications that would be send via the notification
/// channel, those block tasks could then reply to those notifications via the messaging sender channel.
/// The engine would then listen to the sender channel to receive messages from multiple blocks.
struct BlockMessaging {
    /// Channel used be engine to broadcast messages to multiple block tasks
    notifications: watch::Sender<String>,
    /// Pair of channels, sender would be used by block tasks to send messages to engine,
    /// receiver would be used by engine to listen from messages from block tasks.
    messaging: EngineMessaging,
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
    engine_messaging: EngineMessaging,
    /// External senders that would be interested in receiving messages from the engine
    notification_listeners: BTreeMap<uuid::Uuid, Sender<String>>,
    /// Internal messaging for used to communicate
    /// with the block tasks
    block_messaging: BlockMessaging,
}

impl Engine {
    /// Construct
    pub fn new() -> Self {
        let (engine_sender, engine_receiver) = mpsc::channel(32);
        let (block_sender, _) = watch::channel::<String>("".into());

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
                messaging: {
                    let (sender, receiver) = mpsc::channel(32);
                    EngineMessaging { sender, receiver }
                },
            },
        }
    }

    /// Schedule a block to be executed by this engine
    pub fn schedule<B: Block + 'static>(&mut self, mut block: B) {
        self.blocks
            .insert(block.id().to_string(), block.desc().clone());

        let receiver = self.block_messaging.notifications.subscribe();
        let sender = self.block_messaging.messaging.sender.clone();

        self.local.spawn_local(async move {
            loop {
                block.execute().await;

                if receiver.has_changed().is_ok() {
                    let _ = sender.try_send("Block Pong!".to_string());
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
        sender: Sender<String>,
    ) -> Sender<String> {
        self.notification_listeners.insert(sender_id, sender);

        self.engine_messaging.sender.clone()
    }

    /// Runs the event loop of this engine
    /// an execute the blocks that where scheduled
    pub async fn run(&mut self) {
        loop {
            self.local
                .run_until(async {
                    let _ = self.engine_messaging.receiver.recv().await;

                    let _ = self
                        .block_messaging
                        .notifications
                        .send("Block Ping!".into());

                    let _ = self.block_messaging.messaging.receiver.try_recv();

                    for sender in self.notification_listeners.values() {
                        let _ = sender.try_send("Pong!".into());
                    }
                })
                .await
        }
    }
}
