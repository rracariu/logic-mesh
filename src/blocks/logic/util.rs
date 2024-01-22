// Copyright (c) 2022-2024, Radu Racariu.

use libhaystack::val::Bool;
use libhaystack::val::Value;

use crate::base::block::{convert_value, Block};
use crate::base::input::input_reader::InputReader;

/// Converts the inputs to the same type.
pub(super) fn convert_inputs<B: Block>(block: &B) -> (Option<Value>, Option<Value>) {
    let in1 = block.inputs().first().and_then(|input| input.get_value());
    let in2 = block.inputs().get(1).and_then(|input| input.get_value());

    let in2 = in2.and_then(|in2| in1.and_then(|in1| convert_value(in1, in2.clone()).ok()));

    (in1.cloned(), in2)
}

/// Executes the block.
pub(super) async fn execute_impl<B: Block>(
    block: &mut B,
    func: impl Fn(Option<Value>, Option<Value>) -> bool,
) {
    block.read_inputs_until_ready().await;
    let (input1, input2) = convert_inputs(block);

    block.outputs_mut()[0].set(
        Bool {
            value: func(input1, input2),
        }
        .into(),
    );
}
