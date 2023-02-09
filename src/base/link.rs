// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

#[derive(Default, Debug, Clone, Copy)]
pub enum LinkState {
    #[default]
    Disconnected,
    Connected,
    Error,
}

pub trait Link {
    fn state(&self) -> LinkState;
}

#[derive(Debug)]
pub struct BaseLink<Tx> {
    pub id: Uuid,
    pub target_block_id: Uuid,
    pub target_input: String,
    pub tx: Option<Tx>,
    pub state: LinkState,
}

impl<Tx> Link for BaseLink<Tx> {
    fn state(&self) -> LinkState {
        self.state
    }
}

impl<Tx> BaseLink<Tx> {
    pub fn new(target_block_id: Uuid, target_input: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            target_block_id,
            target_input,
            tx: None,
            state: LinkState::Disconnected,
        }
    }
}
