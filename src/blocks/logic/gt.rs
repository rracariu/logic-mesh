// Copyright (c) 2022-2023, Radu Racariu.

//! Greater-than comparison block.

use crate::base::block::Block;
use crate::base::input::InputReader;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::util::compare;

/// Outputs true if value of the in1 is greater.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "GreaterThan"]
#[category = "logic"]
pub struct GreaterThan {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for GreaterThan {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;
        compare(&self.input1, &self.input2, &mut self.out, |in1, in2| {
            in1 > in2
        });
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::logic::GreaterThan,
    };

    #[tokio::test]
    async fn test_gt_block() {
        let mut block = GreaterThan::new();

        write_block_inputs([(&mut block.input1, 3), (&mut block.input2, 1)]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
