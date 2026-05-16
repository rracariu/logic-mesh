// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::InputProps,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::utils::execute_impl;

/// Outputs bitwise AND operation.
#[block]
#[derive(BlockProps, Debug)]
#[category = "bitwise"]
pub struct BitwiseAnd {
    #[input(kind = "Number")]
    pub in1: InputImpl,
    #[input(kind = "Number")]
    pub in2: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for BitwiseAnd {
    async fn execute(&mut self) {
        execute_impl(self, |in1, in2| in1 & in2).await;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::bitwise::BitwiseAnd,
    };

    #[tokio::test]
    async fn test_and_op() {
        let mut block = BitwiseAnd::new();

        write_block_inputs([(&mut block.in1, 2), (&mut block.in2, 2)]).await;

        block.execute().await;

        assert_eq!(block.out.value, (2).into());

        write_block_inputs([(&mut block.in1, 1), (&mut block.in2, 2)]).await;

        block.execute().await;

        assert_eq!(block.out.value, (0).into());
    }
}
