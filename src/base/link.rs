// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum LinkState {
    #[default]
    Disconnected,
    Connected,
    Error,
}

pub trait Link {
    fn id(&self) -> &Uuid;
    fn state(&self) -> LinkState;
    fn target_block_id(&self) -> &Uuid;
    fn target_input(&self) -> &str;
}

#[derive(Debug, PartialEq)]
pub struct BaseLink<Tx> {
    pub id: Uuid,
    pub target_block_id: Uuid,
    pub target_input: String,
    pub tx: Option<Tx>,
    pub state: LinkState,
}

impl<Tx> Link for BaseLink<Tx> {
    fn id(&self) -> &Uuid {
        &self.id
    }

    fn target_block_id(&self) -> &Uuid {
        &self.target_block_id
    }

    fn target_input(&self) -> &str {
        &self.target_input
    }

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
