#[derive(Default, Debug)]
pub enum LinkState {
    #[default]
    Disconnected,
    Connected,
    Error,
}

#[derive(Debug)]
pub struct Link<Tx> {
    pub tx: Option<Tx>,
    pub state: LinkState,
}

impl<Tx> Link<Tx> {
    pub fn new() -> Self {
        Self {
            tx: None,
            state: LinkState::Disconnected,
        }
    }
}
