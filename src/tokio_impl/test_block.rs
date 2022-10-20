use libhaystack::val::Value;

use crate::base::{
    block::Block,
    input::Input,
    output::{Output},
};

use super::{input::InputImpl, output::OutputImpl};

pub struct TestBlock {
    pub input_a: InputImpl,
    pub input_b: InputImpl,
    pub out: OutputImpl,
}

impl TestBlock {
    pub fn new() -> Self {
        TestBlock {
            input_a: InputImpl::new(),
            input_b: InputImpl::new(),
            out: OutputImpl::default(),
        }
    }

    pub async fn execute(&mut self) {
        let a = self.input_a.rx.as_mut().unwrap();
        let b = self.input_b.rx.as_mut().unwrap();

        let (i, val) = tokio::select!(x = a.recv() => (&mut self.input_a, x), x = b.recv() => (&mut self.input_a, x));
        i.val = val;

        let mut out_val = Value::default();

        if let (Some(Value::Number(a)), Some(Value::Number(b))) =
            (&self.input_a.val, &self.input_b.val)
        {
            out_val = (*a + *b).unwrap().into();
        } else if let Some(Value::Number(a)) =
            self.input_a.val.as_ref().or(self.input_b.val.as_ref())
        {
            out_val = (*a + 2.0.into()).unwrap().into();
        }

        println!("Value of out is: {out_val}");
        self.out.set(out_val).await;
    }
}

impl Block for TestBlock {
    type Rx = <InputImpl as Input>::Rx;
    type Tx = <InputImpl as Input>::Tx;

    fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
        todo!()
    }

    fn output(&self) -> &dyn Output {
        &self.out
    }
}
