// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

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
            messages::{
                BlockData, BlockInputData, BlockOutputData, ChangeSource, EngineMessage,
                WatchMessage,
            },
            Engine,
        },
        link::{BaseLink, LinkState},
    },
    blocks::registry::schedule_block,
};

use super::block_pointer::BlockPropsPointer;

#[derive(Default)]
struct WatchSet {
    sender: Option<Sender<WatchMessage>>,
    subjects: BTreeSet<String>,
}

// The concrete trait for the block properties
pub(super) trait BlockPropsType = BlockProps<Write = Sender<Value>, Read = Receiver<Value>>;

type Messages = EngineMessage<Sender<WatchMessage>>;

/// Creates an execution environment for Blocks to be run on.
///
/// Each block would be executed inside a local task in the engine's local context.
///
pub struct LocalSetEngine {
    /// Use to schedule task on the current thread
    local: LocalSet,
    /// Blocks registered with this engine, indexed by block id
    block_props: BTreeMap<Uuid, Rc<Cell<BlockPropsPointer>>>,
    /// Messaging channel used by external processes to control
    /// and inspect this engines execution
    sender: Sender<Messages>,
    // Multi-producer single-consumer channel for receiving messages
    receiver: Receiver<Messages>,
    /// Senders used to reply to issued commands
    /// Each sender would be associated to an external process
    /// issuing commands to the engine.
    reply_senders: BTreeMap<uuid::Uuid, Sender<Messages>>,
    /// Watches for changes in block pins
    watches: BTreeMap<Uuid, Rc<WatchSet>>,
}

impl Engine for LocalSetEngine {
    type Write = Sender<Value>;
    type Read = Receiver<Value>;

    type Sender = Sender<Messages>;

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
        let props = Rc::new(Cell::new(BlockPropsPointer::new(
            &mut block as &mut dyn BlockPropsType,
        )));
        self.block_props.insert(*block.id(), props.clone());

        let watch_set = Rc::new(WatchSet::default());
        self.watches.insert(*block.id(), watch_set.clone());

        self.local.spawn_local(async move {
            // Must do here also so we get the correct address
            // of the moved block instance
            props.set(BlockPropsPointer::new(
                &mut block as &mut dyn BlockPropsType,
            ));

            // Tacks changes to block pins
            let mut last_pin_values = BTreeMap::<String, Value>::new();

            loop {
                block.execute().await;

                change_of_value_check(&watch_set, &block, &mut last_pin_values);

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
                    engine_msg = self.receiver.recv().await;
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
        self.reply_senders.insert(sender_id, sender);

        self.sender.clone()
    }
}

impl LocalSetEngine {
    /// Construct
    pub fn new() -> Self {
        // Create a multi-producer single-consumer channel with a buffer of 32 messages
        let (sender, receiver) = mpsc::channel(32);

        Self {
            local: LocalSet::new(),
            sender,
            receiver,
            block_props: BTreeMap::new(),
            reply_senders: BTreeMap::new(),
            watches: BTreeMap::new(),
        }
    }

    async fn dispatch_message(&mut self, msg: Messages) {
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

    fn reply_to_sender(&mut self, sender_uuid: Uuid, engine_message: Messages) {
        for (sender_id, sender) in self.reply_senders.iter() {
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
        self.watches.remove(block_id);
        res
    }
}

impl Default for LocalSetEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Implements the logic for checking if the watched block pins
/// have changed, and if so, dispatches a message to the watch sender.
fn change_of_value_check<B: Block + 'static>(
    watch_set: &Rc<WatchSet>,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
) {
    // Nothing to do if there are no subjects.
    if watch_set.subjects.is_empty() {
        // Clear the last value if there are no more subjects.
        if !last_pin_values.is_empty() {
            last_pin_values.clear();
        }

        return;
    }

    if let Some(sender) = &watch_set.sender {
        let mut changes = BTreeMap::<String, ChangeSource>::new();

        watch_set.subjects.iter().for_each(|pin| {
            if let Some(out) = block.get_output(pin) {
                let val = out.value();
                if last_pin_values.get(pin) != Some(val) {
                    changes.insert(pin.clone(), ChangeSource::Output(pin.clone(), val.clone()));
                    last_pin_values.insert(pin.clone(), val.clone());
                }
            } else if let Some(input) = block.get_input(pin) {
                if let Some(val) = input.get_value() {
                    if last_pin_values.get(pin) != Some(val) {
                        changes.insert(pin.clone(), ChangeSource::Input(pin.clone(), val.clone()));
                        last_pin_values.insert(pin.clone(), val.clone());
                    }
                }
            }
        });

        if !changes.is_empty() {
            let _ = sender.try_send(WatchMessage { changes });
        }
    }
}
