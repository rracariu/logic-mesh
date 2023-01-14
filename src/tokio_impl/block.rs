use futures::future::select_all;

use crate::base::block::Block;

pub async fn read_block_inputs<B: Block>(block: &mut B) {
    let input_futures = block
        .inputs()
        .into_iter()
        .map(|input| input.receiver())
        .collect::<Vec<_>>();

    let (val, idx, _) = select_all(input_futures).await;

    let mut inputs = block.inputs();
    if let Some(input) = inputs.get_mut(idx) {
        if let Some(value) = val {
            input.set_value(value)
        }
    }
}
