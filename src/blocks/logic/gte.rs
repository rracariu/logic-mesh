// Copyright (c) 2022-2023, Radu Racariu.

//! Greater-than-or-equal comparison block.

use crate::base::block::Block;
use crate::base::input::InputReader;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::util::compare;

/// Outputs true if value of the in1 is greater or equals.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "GreaterThanEqual"]
#[category = "logic"]
pub struct GreaterThanEq {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for GreaterThanEq {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;
        compare(&self.input1, &self.input2, &mut self.out, |in1, in2| {
            in1 >= in2
        });
    }
}
#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::logic::GreaterThanEq,
    };

    #[tokio::test]
    async fn test_gte_block() {
        let mut block = GreaterThanEq::new();

        write_block_inputs([(&mut block.input1, 3), (&mut block.input2, -3)]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
