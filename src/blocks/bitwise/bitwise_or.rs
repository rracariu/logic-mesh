// Copyright (c) 2022-2024, Radu Racariu.

//! Bitwise OR block.

use crate::base::block::Block;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::utils::execute_impl;

/// Outputs bitwise OR operation.
#[block]
#[derive(BlockProps, Debug)]
#[category = "bitwise"]
pub struct BitwiseOr {
    #[input(kind = "Number")]
    pub in1: InputImpl,
    #[input(kind = "Number")]
    pub in2: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for BitwiseOr {
    async fn execute(&mut self) {
        execute_impl(self, |in1, in2| in1 | in2).await;
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::bitwise::BitwiseOr,
    };

    #[tokio::test]
    async fn test_or_op() {
        let mut block = BitwiseOr::new();

        write_block_inputs([(&mut block.in1, 5), (&mut block.in2, 2)]).await;

        block.execute().await;

        assert_eq!(block.out.value, 7.into());

        write_block_inputs([(&mut block.in1, 1), (&mut block.in2, 0)]).await;

        block.execute().await;

        assert_eq!(block.out.value, 1.into());
    }
}
