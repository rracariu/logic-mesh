use uuid::Uuid;

use super::input::{Input, InputReceiver};
use super::output::Output;

#[derive(Default, Debug, Clone, Copy)]
pub enum BlockState {
    #[default]
    Stopped,
    Running,
    Fault,
}

pub struct BlockDesc {
    pub name: String,
    pub library: String,
}

pub trait BlockProps {
    type Rx;
    type Tx;

    fn id(&self) -> &Uuid;

    fn desc(&self) -> &BlockDesc;

    fn state(&self) -> BlockState;

    fn inputs(&mut self) -> Vec<&mut dyn (InputReceiver<Rx = Self::Rx, Tx = Self::Tx>)>;

    fn output(&self) -> &dyn Output;
}

pub trait Block: BlockProps {
    async fn execute(&mut self);
}
