use libhaystack::val::Value;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputReceiver},
    link::{BaseLink, LinkState},
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
    type Rx = <InputImpl as Input>::Rx;
    type Tx = <InputImpl as Input>::Tx;

    fn id(&self) -> &Uuid {
        &self.id
    }

    fn desc(&self) -> &BlockDesc {
        &self.desc
    }

    fn state(&self) -> BlockState {
        BlockState::Running
    }

    fn inputs(&mut self) -> Vec<&mut dyn InputReceiver<Rx = Self::Rx, Tx = Self::Tx>> {
        vec![&mut self.period]
    }

    fn output(&self) -> &dyn Output {
        &self.out
    }
}

impl Block for SineWave {
    async fn execute(&mut self) {}
}
