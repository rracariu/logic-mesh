use tokio::time::{sleep, Duration};

use libhaystack::val::{kind::HaystackKind, Value};
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use super::{block::read_block_inputs, input::InputImpl, output::OutputImpl};

pub struct TestAddBlock {
    id: Uuid,
    desc: BlockDesc,
    pub input_a: InputImpl,
    pub input_b: InputImpl,
    pub out: OutputImpl,
}

impl BlockProps for TestAddBlock {
    type Rx = <InputImpl as InputProps>::Rx;
    type Tx = <InputImpl as InputProps>::Tx;

    fn id(&self) -> &uuid::Uuid {
        &self.id
    }

    fn desc(&self) -> &BlockDesc {
        &self.desc
    }

    fn state(&self) -> BlockState {
        BlockState::Running
    }

    fn inputs(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
        vec![&mut self.input_a, &mut self.input_b]
    }

    fn output(&mut self) -> &mut dyn Output<Tx = Self::Tx> {
        &mut self.out
    }
}

impl Block for TestAddBlock {
    async fn execute(&mut self) {
        read_block_inputs(self).await;

        if self.input_a.val.is_some() || self.input_b.val.is_some() {
            let res = get_num(&self.input_a.val) + get_num(&self.input_b.val);

            println!(
                "Block name: {}, in1: {:?}, in2: {:?} - out {}",
                self.desc.name, self.input_a.val, self.input_b.val, res
            );
            sleep(Duration::from_millis(400)).await;
            self.out.set(res.into()).await;
        }
    }
}

impl TestAddBlock {
    pub fn new(name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            desc: BlockDesc {
                name: name.into(),
                library: "".into(),
            },
            input_a: InputImpl::new("a", HaystackKind::Number),
            input_b: InputImpl::new("b", HaystackKind::Number),
            out: OutputImpl::new(HaystackKind::Number),
        }
    }
}

fn get_num(val: &Option<Value>) -> f64 {
    val.as_ref().map_or(0f64, |v| match v {
        Value::Number(n) => n.value,
        _ => 0f64,
    })
}
