// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use libhaystack::val::Value;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::LocalSet,
};
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps},
    engine_messages::{BlockData, BlockInputData, BlockOutputData},
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

// The concrete trait for the block properties
trait BlockPropsType = BlockProps<Tx = Sender<Value>, Rx = Receiver<Value>>;

/// Creates an execution environment for Blocks to be run on.
///
/// Each block would be executed inside a local task in the engine's local context.
///
pub struct Engine {
    /// Use to schedule task on the current thread
    local: LocalSet,
    /// Blocks descriptions for the blocks registered with this engine, indexed by block id
    blocks_desc: BTreeMap<Uuid, &'static BlockDesc>,
    /// Blocks registered with this engine, indexed by block id
    block_props: BTreeMap<Uuid, Rc<Cell<BlockPropsPointer>>>,
    /// Messaging used by external users to control
    /// and inspect this engines execution
    engine_messaging: EngineMessaging,
    /// External senders that would be interested in receiving messages from the engine
    notification_listeners: BTreeMap<uuid::Uuid, Sender<EngineMessage>>,
}

impl Engine {
    /// Construct
    pub fn new() -> Self {
        let (engine_sender, engine_receiver) = mpsc::channel(32);

        Self {
            local: LocalSet::new(),
            blocks_desc: BTreeMap::default(),
            block_props: BTreeMap::default(),
            engine_messaging: EngineMessaging {
                sender: engine_sender,
                receiver: engine_receiver,
            },
            notification_listeners: BTreeMap::default(),
        }
    }

    /// Schedule a block to be executed by this engine
    pub fn schedule<B: Block<Tx = Sender<Value>, Rx = Receiver<Value>> + 'static>(
        &mut self,
        mut block: B,
    ) {
        self.blocks_desc.insert(*block.id(), block.desc());
        self.block_props.insert(*block.id(), Rc::default());

        let props = self.block_props.get_mut(block.id()).unwrap().clone();

        self.local.spawn_local(async move {
            let block_props = &block as &dyn BlockPropsType;
            let block_props_ptr = block_props as *const (dyn BlockPropsType + 'static);

            props.set(BlockPropsPointer::new(block_props_ptr));

            loop {
                block.execute().await;
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

            EngineMessage::InspectBlockReq(sender_uuid, block_uuid) => {
                if let Some(block) = self.get_block_props_mut(&block_uuid) {
                    let data = BlockData {
                        id: block.id().to_string(),
                        name: block.desc().name.clone(),
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

                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::FoundBlockRes(sender_uuid, Some(data)),
                    );
                } else {
                    self.reply_to_sender(
                        sender_uuid,
                        EngineMessage::FoundBlockRes(sender_uuid, None),
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
                    source_block.output_mut().add_link(link);

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
        self.block_props.get_mut(block_id).and_then(|ptr| unsafe {
            let fat_ptr = (**ptr).get();
            fat_ptr.get().map(|ptr| &mut *ptr)
        })
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

/// Holds a fat pointer to a BlockProps
/// trait object.
#[derive(Default, Clone, Copy)]
struct BlockPropsPointer {
    fat_pointer: [usize; 2],
}

impl BlockPropsPointer {
    /// Constructs the BlockProps pointer from a raw pointer to the trait
    /// object.
    fn new(ptr: *const dyn BlockPropsType) -> Self {
        let ptr_ref = &ptr as *const *const dyn BlockPropsType;
        let pointer_parts = ptr_ref as *const [usize; 2];

        let fat_pointer = unsafe { *pointer_parts };
        Self { fat_pointer }
    }

    /// Tries to get the pointer to the trait object from the fat pointer
    /// stored.
    /// It returns None if there is no pointer store.
    ///
    /// # Safety
    /// This would be unsafe if the pointer stored is no longer valid.
    fn get(&self) -> Option<*mut dyn BlockPropsType> {
        if self.fat_pointer == [0; 2] {
            None
        } else {
            let ptr: *mut dyn BlockPropsType = unsafe {
                let pointer_parts: *const [usize; 2] = &self.fat_pointer;
                let ptr_ref = pointer_parts as *const *mut dyn BlockPropsType;
                *ptr_ref
            };
            unsafe { Some(&mut *ptr) }
        }
    }
}
