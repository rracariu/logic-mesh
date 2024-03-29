// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Single threaded engine implementation
//!
//! Spawn a local task for each block to be executed on the current thread.

use std::{cell::Cell, cell::RefCell, collections::BTreeMap, rc::Rc};

use anyhow::{anyhow, Result};
use libhaystack::val::Value;

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::LocalSet,
};
use uuid::Uuid;

use super::message_dispatch::dispatch_message;
use super::{block_pointer::BlockPropsPointer, schedule_block_on_engine};
use crate::{
    base::{
        block::{
            connect::{connect_input, connect_output, disconnect_block},
            Block, BlockProps, BlockState,
        },
        engine::{
            messages::{ChangeSource, EngineMessage, WatchMessage},
            Engine,
        },
        program::data::{BlockData, LinkData},
    },
    blocks::registry::get_block,
    tokio_impl::input::{Reader, Writer},
};

// The concrete trait for the block properties
pub(super) trait BlockPropsType = BlockProps<Writer = Writer, Reader = Reader>;

/// The concrete type for the engine messages
pub type Messages = EngineMessage<Sender<WatchMessage>>;

/// Creates single threaded execution environment for Blocks to be run on.
///
/// Each block would be executed inside a local task in the engine's local context.
///
pub struct SingleThreadedEngine {
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
    pub(super) reply_senders: BTreeMap<uuid::Uuid, Sender<Messages>>,
    /// Watchers for changes in block pins
    pub(super) watchers: Rc<RefCell<BTreeMap<Uuid, Sender<WatchMessage>>>>,
}

impl Default for SingleThreadedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine for SingleThreadedEngine {
    type Writer = Writer;
    type Reader = Reader;

    type Channel = Sender<Messages>;

    fn schedule<B: Block<Writer = Self::Writer, Reader = Self::Reader> + 'static>(
        &mut self,
        mut block: B,
    ) {
        let props = Rc::new(Cell::new(BlockPropsPointer::new(
            &mut block as &mut dyn BlockPropsType,
        )));
        self.block_props.insert(*block.id(), props.clone());

        let watchers = self.watchers.clone();

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

                change_of_value_check(&watchers, &block, &mut last_pin_values);

                if block.state() == BlockState::Terminated {
                    break;
                }
            }
        });
    }

    fn load_blocks_and_links(&mut self, blocks: &[BlockData], links: &[LinkData]) -> Result<()> {
        blocks.iter().try_for_each(|block| -> Result<()> {
            let id = Uuid::try_from(block.id.as_str()).ok();
            if id.is_none() {
                return Err(anyhow!("Invalid block id"));
            }

            let block = get_block(&block.name, Some(block.lib.clone()))
                .ok_or_else(|| anyhow!("Block not found"))?;
            schedule_block_on_engine(&block.desc, id, self)?;

            Ok(())
        })?;

        links
            .iter()
            .try_for_each(|link| self.connect_blocks(link).map(|_| ()))
    }

    async fn run(&mut self) {
        let mut is_paused = false;
        loop {
            let local_tasks = &self.local;
            let mut engine_msg = None;

            if !is_paused {
                local_tasks
                    .run_until(async {
                        engine_msg = self.receiver.recv().await;
                    })
                    .await;
            } else {
                engine_msg = self.receiver.recv().await;
            }

            if let Some(message) = engine_msg {
                if matches!(message, EngineMessage::Shutdown) {
                    break;
                } else if matches!(message, EngineMessage::Reset) {
                    self.blocks_iter_mut().for_each(|block| {
                        block.set_state(BlockState::Terminated);
                    });

                    self.block_props.clear();
                    continue;
                } else if matches!(message, EngineMessage::Pause) {
                    is_paused = true;
                    continue;
                } else if matches!(message, EngineMessage::Resume) {
                    is_paused = false;
                    continue;
                }

                dispatch_message(self, message).await;
            }
        }
    }

    fn create_message_channel(
        &mut self,
        sender_id: uuid::Uuid,
        sender_channel: Self::Channel,
    ) -> Self::Channel {
        self.reply_senders.insert(sender_id, sender_channel);

        self.sender.clone()
    }
}

impl SingleThreadedEngine {
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
            watchers: Rc::default(),
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

    pub(super) fn blocks_iter_mut(&self) -> impl Iterator<Item = &mut dyn BlockPropsType> {
        self.block_props
            .values()
            .filter_map(|props| {
                let props = props.get();
                props.get()
            })
            .map(|prop| unsafe { &mut *prop })
    }

    pub(super) fn connect_blocks(&mut self, link_data: &LinkData) -> Result<LinkData> {
        let (source_block_uuid, target_block_uuid) = (
            Uuid::try_from(link_data.source_block_uuid.as_str())?,
            Uuid::try_from(link_data.target_block_uuid.as_str())?,
        );

        let Some(source_block) = self.get_block_props_mut(&source_block_uuid) else {
            return Err(anyhow!(
                "Source block '{}' not found",
                link_data.source_block_uuid
            ));
        };
        let Some(target_block) = self.get_block_props_mut(&target_block_uuid) else {
            return Err(anyhow!(
                "Target block '{}' not found",
                link_data.target_block_uuid
            ));
        };

        let Some(target_input) = target_block.get_input_mut(&link_data.target_block_pin_name)
        else {
            return Err(anyhow!(
                "Target input pin '{}' not found",
                link_data.target_block_pin_name
            ));
        };

        let link_id;
        if let Some(source_input) = source_block.get_input_mut(&link_data.source_block_pin_name) {
            link_id = connect_input(source_input, target_input).map_err(|err| anyhow!(err))?;

            if let Some(val) = source_input.get_value() {
                target_input
                    .writer()
                    .try_send(val.clone())
                    .map_err(|err| anyhow!(err))?;
            }
            reset_connected_inputs(target_block, &link_data.target_block_pin_name)?;
        } else if let Some(source_output) =
            source_block.get_output_mut(&link_data.source_block_pin_name)
        {
            link_id = connect_output(source_output, target_input).map_err(|err| anyhow!(err))?;

            // After connection, send the current value of the source output to the target input
            if source_output.value().has_value() {
                // Send the current value of the source output to the target input
                target_input
                    .writer()
                    .try_send(source_output.value().clone())
                    .map_err(|err| anyhow!(err))?;
            }
            reset_connected_inputs(target_block, &link_data.target_block_pin_name)?;
        } else {
            return Err(anyhow!(
                "Source pin '{}' not found",
                link_data.source_block_pin_name
            ));
        }
        Ok(LinkData {
            id: Some(link_id.to_string()),
            ..link_data.clone()
        })
    }

    pub(super) fn save_blocks_and_links(&mut self) -> Result<(Vec<BlockData>, Vec<LinkData>)> {
        let blocks = self
            .blocks_iter_mut()
            .map(|block| BlockData {
                id: block.id().to_string(),
                name: block.name().to_string(),
                dis: block.desc().dis.to_string(),
                lib: block.desc().library.clone(),
                category: block.desc().category.clone(),
                ver: block.desc().ver.clone(),
            })
            .collect();

        let mut links: Vec<LinkData> = Vec::new();
        for block in self.blocks_iter_mut() {
            for (pin_name, pin_links) in block.links() {
                for link in pin_links {
                    links.push(LinkData {
                        id: Some(link.id().to_string()),
                        source_block_pin_name: pin_name.to_string(),
                        source_block_uuid: block.id().to_string(),
                        target_block_pin_name: link.target_input().to_string(),
                        target_block_uuid: link.target_block_id().to_string(),
                    });
                }
            }
        }

        Ok((blocks, links))
    }

    pub(super) fn get_block_props_mut(
        &self,
        block_id: &Uuid,
    ) -> Option<&mut (dyn BlockPropsType + 'static)> {
        self.block_props.get(block_id).and_then(|ptr| {
            let fat_ptr = (**ptr).get();
            fat_ptr.get().map(|ptr| unsafe { &mut *ptr })
        })
    }

    pub(super) fn add_block(
        &mut self,
        block_name: String,
        block_id: Option<Uuid>,
        lib: Option<String>,
    ) -> Result<Uuid> {
        let block =
            get_block(block_name.as_str(), lib).ok_or_else(|| anyhow!("Block not found"))?;

        schedule_block_on_engine(&block.desc, block_id, self)
    }

    pub(super) fn remove_block(&mut self, block_id: &Uuid) -> Result<Uuid> {
        // Terminate the block
        match self.get_block_props_mut(block_id) {
            Some(block) => {
                block.set_state(BlockState::Terminated);

                disconnect_block(block, |id, name| {
                    self.decrement_refresh_block_input(id, name)
                });
            }
            None => return Err(anyhow!("Block not found")),
        };

        // Remove the block from any links
        self.blocks_iter_mut().for_each(|block| {
            if block.id() == block_id {
                return;
            }

            let mut outs = block.outputs_mut();
            outs.iter_mut().for_each(|output| {
                output.remove_target_block_links(block_id);
            });

            let mut ins = block.inputs_mut();
            ins.iter_mut().for_each(|input| {
                input.remove_target_block_links(block_id);
            });
        });

        // Remove the block from the block props
        self.block_props.remove(block_id);

        Ok(*block_id)
    }

    /// Decrements the connection count of the target block input.
    /// Sends the current value of the input to itself (refresh current value.)
    pub(crate) fn decrement_refresh_block_input(
        &self,
        block_id: &Uuid,
        input_name: &str,
    ) -> Option<usize> {
        let target_block = self.get_block_props_mut(block_id);
        target_block.and_then(|target_block| {
            target_block.get_input_mut(input_name).map(|input| {
                let cnt = input.decrement_conn();
                let value = input.get_value().cloned();
                input.writer().try_send(value.unwrap_or_default()).ok();

                cnt
            })
        })
    }
}

/// Implements the logic for checking if the watched block pins
/// have changed, and if so, dispatches a message to the watch sender.
fn change_of_value_check<B: Block + 'static>(
    notification_channels: &Rc<RefCell<BTreeMap<Uuid, Sender<WatchMessage>>>>,
    block: &B,
    last_pin_values: &mut BTreeMap<String, Value>,
) {
    if notification_channels.borrow().is_empty() {
        if !last_pin_values.is_empty() {
            last_pin_values.clear();
        }
        return;
    }

    let mut changes = BTreeMap::<String, ChangeSource>::new();

    block.outputs().iter().for_each(|output| {
        let pin = output.desc().name.to_string();
        let val = output.value();
        if last_pin_values.get(&pin) != Some(val) {
            changes.insert(pin.clone(), ChangeSource::Output(pin.clone(), val.clone()));
            last_pin_values.insert(pin, val.clone());
        }
    });

    block.inputs().iter().for_each(|input| {
        let val = input.get_value();
        if let Some(val) = val {
            let pin = input.name().to_string();
            if last_pin_values.get(&pin) != Some(val) {
                changes.insert(pin.clone(), ChangeSource::Input(pin.clone(), val.clone()));
                last_pin_values.insert(pin, val.clone());
            }
        }
    });

    if !changes.is_empty() {
        for sender in notification_channels.borrow().values() {
            let _ = sender.try_send(WatchMessage {
                block_id: *block.id(),
                changes: changes.clone(),
            });
        }
    }
}

/// Implements the logic for resetting the target block inputs when a new link is created.
/// This is needed because the target block would be monitoring the current set of inputs
/// added before the link was created, and would not be aware of the newly created link.
fn reset_connected_inputs(
    target_block: &mut dyn BlockPropsType,
    input_to_ignore: &str,
) -> Result<()> {
    // If the target block has other connected inputs, send the current value of one of
    // them to itself (refresh current value.) in order to trigger the block to execute.
    if let Some(a_connected_input) = target_block.inputs().iter().find_map(|input| {
        if input.is_connected() && input.name() != input_to_ignore {
            Some(input.name().to_string())
        } else {
            None
        }
    }) {
        if let Some(target_input) = target_block.get_input_mut(&a_connected_input) {
            if let Some(value) = target_input.get_value().cloned() {
                target_input
                    .writer()
                    .try_send(value.clone())
                    .map_err(|err| anyhow!(err))?;
            }
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test {
    use std::{thread, time::Duration};

    use crate::base;
    use crate::blocks::{math::Add, misc::SineWave};
    use base::block::{BlockConnect, BlockProps};
    use base::engine::messages::EngineMessage::{InspectBlockReq, InspectBlockRes, Shutdown};

    use crate::tokio_impl::engine::single_threaded::SingleThreadedEngine;
    use base::engine::Engine;
    use tokio::{runtime::Runtime, sync::mpsc, time::sleep};
    use uuid::Uuid;

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

        let mut eng = SingleThreadedEngine::new();

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

                    if let Some(InspectBlockRes(Ok(data))) = res {
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
