use super::input::Input;
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
}

pub trait Block {
    type Rx;
    type Tx;

    fn desc(&self) -> &BlockDesc;

    fn state(&self) -> BlockState;

    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>>;

    fn output(&self) -> &dyn Output;
}
