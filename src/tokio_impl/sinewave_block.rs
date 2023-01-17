use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use super::{input::InputImpl, output::OutputImpl};

pub struct SineWave {
    id: Uuid,
    pub period: InputImpl,
    pub out: OutputImpl,
    desc: BlockDesc,
}

impl BlockProps for SineWave {
    type Rx = <InputImpl as InputProps>::Rx;
    type Tx = <InputImpl as InputProps>::Tx;

    fn id(&self) -> &Uuid {
        &self.id
    }

    fn desc(&self) -> &BlockDesc {
        &self.desc
    }

    fn state(&self) -> BlockState {
        BlockState::Running
    }

    fn inputs(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
        vec![&mut self.period]
    }

    fn output(&mut self) -> &mut dyn Output<Tx = Self::Tx> {
        &mut self.out
    }
}

impl Block for SineWave {
    async fn execute(&mut self) {}
}
