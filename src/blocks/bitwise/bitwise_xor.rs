// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

use super::common::BitwiseOperator;

/// Outputs bitwise XOR operation.
#[block]
#[derive(BlockProps, Debug)]
#[category = "bitwise"]
pub struct BitwiseXor {
    #[input(kind = "Number")]
    pub in1: InputImpl,
    #[input(kind = "Number")]
    pub in2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl BitwiseOperator for BitwiseXor {
    fn calculate(in1: i64, in2: i64) -> i64 {
        in1 ^ in2
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::Block,
        base::{block::test_utils::write_block_inputs, input::input_reader::InputReader},
        blocks::bitwise::BitwiseXor,
    };

    #[tokio::test]
    async fn test_and_op() {
        let mut block = BitwiseXor::new();

        for _ in
            write_block_inputs(&mut [(&mut block.in1, 2.into()), (&mut block.in2, 2.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_eq!(block.out.value, (0).into());

        for _ in
            write_block_inputs(&mut [(&mut block.in1, 4.into()), (&mut block.in2, 2.into())]).await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_eq!(block.out.value, (6).into());
    }
}
