use uuid::Uuid;

use super::input::{Input, InputReceiver};
use super::output::OutputLink;

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
    type Tx: Clone;

    fn id(&self) -> &Uuid;

    fn desc(&self) -> &BlockDesc;

    fn state(&self) -> BlockState;

    fn inputs(&mut self) -> Vec<&mut dyn (InputReceiver<Rx = Self::Rx, Tx = Self::Tx>)>;

    fn output(&mut self) -> &mut dyn OutputLink<Tx = Self::Tx>;
}

pub trait BlockConnect: BlockProps {
    fn connect<I: Input<Tx = Self::Tx>>(&mut self, input: &mut I);
}

pub trait Block: BlockProps + BlockConnect {
    async fn execute(&mut self);
}
