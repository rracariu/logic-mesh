use futures::future::select_all;

use crate::base::{
    block::{Block, BlockConnect, BlockProps},
    input::Input,
    link::{BaseLink, LinkState},
};

impl<T: Block> BlockConnect for T {
    fn connect<I: Input<Tx = Self::Tx>>(&mut self, input: &mut I) {
        let mut link = BaseLink::<Self::Tx>::new();

        link.tx = Some(input.writer().clone());

        link.state = LinkState::Connected;

        self.output().add_link(link);
    }
}

pub async fn read_block_inputs<B: Block>(block: &mut B)
where
    <B as BlockProps>::Tx: Clone,
{
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
