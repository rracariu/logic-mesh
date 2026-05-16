// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::InputProps,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::util::execute_impl;

/// Outputs true if value of the in1 is less.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "LessThan"]
#[category = "logic"]
pub struct LessThan {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for LessThan {
    async fn execute(&mut self) {
        execute_impl(self, |in1, in2| in1 < in2).await;
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::logic::LessThan,
    };

    #[tokio::test]
    async fn test_lt_block() {
        let mut block = LessThan::new();

        write_block_inputs([(&mut block.input1, 3), (&mut block.input2, 41)]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
