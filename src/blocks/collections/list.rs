// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs a list of elements constructed from the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
#[input(kind = "Null", count = 16)]
pub struct List {
    #[output(kind = "List")]
    pub out: OutputImpl,
}

impl Block for List {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let list = self
            .inputs()
            .into_iter()
            .filter_map(|input| input.get_value().cloned())
            .collect();

        self.out.set(Value::make_list(list));
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::Block, base::block::BlockProps, base::block::test_utils::write_block_inputs,
        blocks::collections::List,
    };

    #[tokio::test]
    async fn test_list_block() {
        let mut block = List::new();

        write_block_inputs(&mut [(block.get_input_mut("in0").unwrap(), 45.into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, vec![45.into()].into());
    }
}
