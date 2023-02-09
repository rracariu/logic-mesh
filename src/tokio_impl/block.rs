// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::future::select_all;
use libhaystack::val::kind::HaystackKind;

use crate::base::block::Block;

pub(crate) async fn read_block_inputs<B: Block>(block: &mut B) {
    let input_futures = block
        .inputs_mut()
        .into_iter()
        .filter(|input| input.is_connected())
        .map(|input| input.receiver())
        .collect::<Vec<_>>();

    let (val, idx, _) = select_all(input_futures).await;

    if let Some(value) = val {
        let mut inputs = block.inputs_mut();
        if let Some(input) = inputs.get_mut(idx) {
            if *input.kind() != HaystackKind::from(&value) {
                block.set_state(crate::base::block::BlockState::Fault);
            } else {
                input.set_value(value)
            }
        }
    }
}
