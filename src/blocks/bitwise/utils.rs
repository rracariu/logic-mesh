// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::block::Block;
use crate::base::block::BlockState;
use crate::base::input::input_reader::InputReader;
use libhaystack::val::Value;

pub(super) async fn execute_impl<B: Block>(block: &mut B, calc: impl Fn(i64, i64) -> i64) {
    block.read_inputs_until_ready().await;

    let inputs = block.inputs();

    if let (Some(Value::Number(in1)), Some(Value::Number(in2))) = (
        inputs[0].get_value().cloned(),
        inputs[1].get_value().cloned(),
    ) {
        block.outputs_mut()[0].set(Value::make_int(calc(in1.value as i64, in2.value as i64)));
        block.set_state(BlockState::Running);
    } else {
        block.set_state(BlockState::Fault);
    }
}
