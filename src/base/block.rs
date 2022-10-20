use super::input::Input;
use super::output::Output;

pub trait Block {
    type Rx;
    type Tx;

    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>>;

    fn output(&self) -> &dyn Output;
}
