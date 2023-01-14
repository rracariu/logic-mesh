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
    pub tx: Option<Tx>,
    pub state: LinkState,
}

impl<Tx> Link for BaseLink<Tx> {
    fn state(&self) -> LinkState {
        self.state
    }
}

impl<Tx> BaseLink<Tx> {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            tx: None,
            state: LinkState::Disconnected,
        }
    }
}
