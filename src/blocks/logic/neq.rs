// Copyright (c) 2022-2023, Radu Racariu.

//! Not-equal comparison block.

use crate::base::block::Block;
use crate::base::input::InputReader;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::util::compare;

/// Outputs true if value of the inputs are not equal.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct NotEqual {
    #[input(name = "in1", kind = "Null")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Null")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for NotEqual {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;
        compare(&self.input1, &self.input2, &mut self.out, |in1, in2| {
            in1 != in2
        });
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::logic::NotEqual,
    };

    #[tokio::test]
    async fn test_neq_block() {
        let mut block = NotEqual::new();

        write_block_inputs([(&mut block.input1, "true"), (&mut block.input2, "false")]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
