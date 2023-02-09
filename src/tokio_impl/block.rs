// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::future::select_all;

use crate::base::{
    block::{Block, BlockConnect},
    input::InputProps,
    link::{BaseLink, LinkState},
};

impl<T: Block> BlockConnect for T {
    fn connect<I: InputProps<Tx = Self::Tx>>(&mut self, input: &mut I) {
        let mut link = BaseLink::<Self::Tx>::new(*input.block_id(), input.name().to_string());

        link.tx = Some(input.writer().clone());

        link.state = LinkState::Connected;

        self.output().add_link(link);
        input.increment_conn();
    }
}

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
