use crate::base::block::Block;
use crate::base::block::BlockProps;
use crate::base::block::BlockState;
use crate::base::block::BlockStaticDesc;
use crate::base::input::input_reader::InputReader;
use libhaystack::val::Value;

/// Implements a Bitwise operator block.
pub(super) trait BitwiseOperator {
    /// Calculates the result of the operator.
    fn calculate(in1: i64, in2: i64) -> i64;
}

/// Implements the operator block.
impl<T: BitwiseOperator + BlockProps + BlockStaticDesc> Block for T {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let inputs = self.inputs();

        if let (Some(Value::Number(in1)), Some(Value::Number(in2))) = (
            inputs[0].get_value().cloned(),
            inputs[1].get_value().cloned(),
        ) {
            self.outputs_mut()[0].set(Value::make_int(Self::calculate(
                in1.value as i64,
                in2.value as i64,
            )));
            self.set_state(BlockState::Running);
        } else {
            self.set_state(BlockState::Fault);
        }
    }
}
