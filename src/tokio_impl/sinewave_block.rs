use crate::base::{
    block::{Block, BlockDesc, BlockState},
    input::Input,
    output::Output,
};

use super::{input::InputImpl, output::OutputImpl};

pub struct SineWave {
    pub period: InputImpl,
    pub out: OutputImpl,
    desc: BlockDesc,
}

impl SineWave {
    pub async fn execute() {}
}

impl Block for SineWave {
    type Rx = <InputImpl as Input>::Rx;
    type Tx = <InputImpl as Input>::Tx;

    fn desc(&self) -> &BlockDesc {
        &self.desc
    }

    fn state(&self) -> BlockState {
        BlockState::Running
    }

    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
        vec![&self.period]
    }

    fn output(&self) -> &dyn Output {
        &self.out
    }
}
