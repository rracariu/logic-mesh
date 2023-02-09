// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::future::select_all;

use crate::base::block::Block;

pub async fn read_block_inputs<B: Block>(block: &mut B) {
    let input_futures = block
        .inputs()
        .into_iter()
        .filter(|input| input.is_connected())
        .map(|input| input.receiver())
        .collect::<Vec<_>>();

    let (val, idx, _) = select_all(input_futures).await;

    if let Some(value) = val {
        let mut inputs = block.inputs();
        if let Some(input) = inputs.get_mut(idx) {
            input.set_value(value)
        }
    }
}
