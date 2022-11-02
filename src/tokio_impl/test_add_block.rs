use tokio::time::{sleep, Duration};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::base::{
    block::{Block, BlockDesc, BlockState},
    input::Input,
    output::Output,
};

use super::{input::InputImpl, output::OutputImpl};
use futures::{future::select_all, FutureExt};

pub struct TestAddBlock {
    desc: BlockDesc,
    pub input_a: InputImpl,
    pub input_b: InputImpl,
    pub out: OutputImpl,
}

impl Block for TestAddBlock {
    type Rx = <InputImpl as Input>::Rx;
    type Tx = <InputImpl as Input>::Tx;

    fn desc(&self) -> &BlockDesc {
        &self.desc
    }

    fn state(&self) -> BlockState {
        BlockState::Running
    }

    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
        vec![&self.input_a, &self.input_b]
    }

    fn output(&self) -> &dyn Output {
        &self.out
    }
}

impl TestAddBlock {
    pub fn new(name: &str) -> Self {
        TestAddBlock {
            desc: BlockDesc { name: name.into() },
            input_a: InputImpl::new_without_default("a", HaystackKind::Number),
            input_b: InputImpl::new_without_default("b", HaystackKind::Number),
            out: OutputImpl::new(HaystackKind::Number),
        }
    }

    pub async fn execute(&mut self) {
        self.read_inputs().await;

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

    async fn read_inputs(&mut self) {
        let a = self.input_a.rx.recv().boxed();
        let b = self.input_b.rx.recv().boxed();

        let (val, idx, _) = select_all(vec![a, b]).await;

        match idx {
            0 => self.input_a.val = val,
            1 => self.input_b.val = val,
            _ => {}
        }
    }
}

fn get_num(val: &Option<Value>) -> f64 {
    val.as_ref().map_or(0f64, |v| match v {
        Value::Number(n) => n.value,
        _ => 0f64,
    })
}
