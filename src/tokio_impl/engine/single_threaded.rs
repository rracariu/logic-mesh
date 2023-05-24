// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use anyhow::{anyhow, Result};
use libhaystack::val::Value;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::LocalSet,
};
use uuid::Uuid;

use crate::{
    base::{
        block::{
            connect::{connect_input, connect_output},
            Block, BlockProps, BlockState,
        },
        engine::{
            messages::{
                BlockInputData, BlockOutputData, BlockParam, ChangeSource, EngineMessage,
                WatchMessage,
            },
            Engine,
        },
        program::data::{BlockData, LinkData},
    },
    blocks::registry::{schedule_block, schedule_block_with_uuid},
};

use super::block_pointer::BlockPropsPointer;

#[derive(Default)]
struct WatchSet {
    sender: Option<Sender<WatchMessage>>,
    subjects: BTreeSet<String>,
}

// The concrete trait for the block properties
pub(super) trait BlockPropsType = BlockProps<Writer = Sender<Value>, Reader = Receiver<Value>>;

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

impl Default for LocalSetEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for LocalSetEngine {
    type Writer = Sender<Value>;
    type Reader = Receiver<Value>;

    type Sender = Sender<Messages>;

    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
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

    fn load_blocks_and_links(&mut self, blocks: &[BlockData], links: &[LinkData]) -> Result<()> {
        blocks.iter().try_for_each(|block| -> Result<()> {
            let id = Uuid::try_from(block.id.as_str())?;
            schedule_block_with_uuid(&block.name, id, self)?;
            Ok(())
        })?;

        links
            .iter()
            .try_for_each(|link| self.connect_blocks(link).map(|_| ()))
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

    /// Get a list of all the blocks that are currently
    /// scheduled on this engine.
    pub fn blocks(&self) -> Vec<&dyn BlockPropsType> {
        self.blocks_iter_mut().map(|prop| &*prop).collect()
    }

    /// Get a list of all the blocks that are currently
    /// scheduled on this engine.
    pub fn blocks_mut(&self) -> Vec<&mut dyn BlockPropsType> {
        self.blocks_iter_mut().collect()
    }

    fn blocks_iter_mut(&self) -> impl Iterator<Item = &mut dyn BlockPropsType> {
        self.block_props
            .values()
            .filter_map(|props| {
                let props = props.get();
                props.get()
            })
            .map(|prop| unsafe { &mut *prop })
    }

    fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData> {
        let (source_block_uuid, target_block_uuid) = (
            Uuid::try_from(link_data.source_block_uuid.as_str())?,
            Uuid::try_from(link_data.target_block_uuid.as_str())?,
        );

        if let (Some(source_block), Some(target_block)) = (
            self.get_block_props_mut(&source_block_uuid),
            self.get_block_props_mut(&target_block_uuid),
        ) {
            if let Some(target_input) =
                target_block.get_input_mut(&link_data.target_block_input_name)
            {
                if let Some(source_input) =
                    source_block.get_input_mut(&link_data.source_block_pin_name)
                {
                    let _ = connect_input(source_input, target_input);
                } else if let Some(source_output) =
                    source_block.get_output_mut(&link_data.source_block_pin_name)
                {
                    let _ = connect_output(source_output, target_input);
                } else {
                    return Err(anyhow!("Source Pin not found"));
                }
                Ok(link_data.clone())
            } else {
                Err(anyhow!("Target Input not found"))
            }
        } else {
            Err(anyhow!("Block not found"))
        }
    }

    fn save_blocks_and_links(&mut self) -> Result<(Vec<BlockData>, Vec<LinkData>)> {
        let blocks = self
            .blocks_iter_mut()
            .map(|bloc| BlockData {
                id: bloc.id().to_string(),
                name: bloc.name().to_string(),
                lib: bloc.desc().library.clone(),
                ver: bloc.desc().ver.clone(),
            })
            .collect();

        let mut links: Vec<LinkData> = Vec::new();
        for block in self.blocks_iter_mut() {
            for (pin_name, pin_links) in block.links() {
                for link in pin_links {
                    links.push(LinkData {
                        source_block_pin_name: pin_name.to_string(),
                        source_block_uuid: block.id().to_string(),
                        target_block_input_name: link.target_input().to_string(),
                        target_block_uuid: link.target_block_id().to_string(),
                    });
                }
            }
        }

        Ok((blocks, links))
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
                    let data = BlockParam {
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
                let res = self.connect_blocks(&link_data);
                self.reply_to_sender(
                    sender_uuid,
                    EngineMessage::ConnectBlocksRes(
                        sender_uuid,
                        res.map_err(|err| err.to_string()),
                    ),
                );
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

    fn get_block_props_mut(&self, block_id: &Uuid) -> Option<&mut (dyn BlockPropsType + 'static)> {
        self.block_props.get(block_id).and_then(|ptr| {
            let fat_ptr = (**ptr).get();
            fat_ptr.get().map(|ptr| unsafe { &mut *ptr })
        })
    }

    fn add_block(&mut self, block_name: String) -> Option<Uuid> {
        schedule_block(&block_name, self).ok()
    }

    fn remove_block(&mut self, block_id: &Uuid) -> Option<Uuid> {
        // Terminate the block
        let res = self.get_block_props_mut(block_id).map(|block| {
            block.set_state(BlockState::Terminate);
            *block.id()
        });

        // Remove the block from any links
        self.blocks_iter_mut().for_each(|block| {
            let mut outs = block.outputs_mut();
            outs.iter_mut().for_each(|output| {
                output
                    .links()
                    .retain(|link| link.target_block_id() != block_id);
            });

            let mut ins = block.inputs_mut();
            ins.iter_mut().for_each(|input| {
                input
                    .links()
                    .retain(|link| link.target_block_id() != block_id);
            });
        });

        // Remove the block from the block props and watches
        self.block_props.remove(block_id);
        self.watches.remove(block_id);
        res
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

#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::base;
    use crate::blocks::{maths::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes, Shutdown};

    use crate::tokio_impl::engine::single_threaded::LocalSetEngine;
    use base::engine::Engine;
    use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
    use uuid::Uuid;

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test(flavor = "current_thread")]
    async fn engine_test() {
        use crate::base::block::connect::connect_output;

        let mut add1 = Add::new();
        let add_uuid = *add1.id();

        let mut sine1 = SineWave::new();

        sine1.amplitude.val = Some(3.into());
        sine1.freq.val = Some(200.into());
        connect_output(&mut sine1.out, add1.inputs_mut()[0]).expect("Connected");

        let mut sine2 = SineWave::new();
        sine2.amplitude.val = Some(7.into());
        sine2.freq.val = Some(400.into());

        sine2
            .connect_output("out", add1.inputs_mut()[1])
            .expect("Connected");

        let mut eng = LocalSetEngine::new();

        let (sender, mut receiver) = mpsc::channel(32);
        let channel_id = Uuid::new_v4();
        let engine_sender = eng.create_message_channel(channel_id, sender.clone());

        thread::spawn(move || {
            let rt = Runtime::new().expect("RT");

            let handle = rt.spawn(async move {
                loop {
                    sleep(Duration::from_millis(300)).await;

                    let _ = engine_sender
                        .send(InspectBlockReq(channel_id, add_uuid))
                        .await;

                    let res = receiver.recv().await;

                    if let Some(InspectBlockRes(id, Some(data))) = res {
                        assert_eq!(id, channel_id);
                        assert_eq!(data.id, add_uuid.to_string());
                        assert_eq!(data.name, "Add");
                        assert_eq!(data.inputs.len(), 16);
                        assert_eq!(data.outputs.len(), 1);
                    } else {
                        assert!(false, "Failed to find block: {:?}", res)
                    }

                    let _ = engine_sender.send(Shutdown).await;
                    break;
                }
            });

            rt.block_on(async { handle.await })
        });

        eng.schedule(add1);
        eng.schedule(sine1);
        eng.schedule(sine2);

        eng.run().await;
    }
}
