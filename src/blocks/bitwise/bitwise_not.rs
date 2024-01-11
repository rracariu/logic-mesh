// Copyright (c) 2022-2024, Radu Racariu.

use libhaystack::val::Value;
use uuid::Uuid;

use crate::base::input::input_reader::InputReader;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs bitwise NOT operation.
#[block]
#[derive(BlockProps, Debug)]
#[category = "bitwise"]
pub struct BitwiseNot {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for BitwiseNot {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(n)) = self.input.get_value() {
            self.out.value = Value::make_int(!(n.value as i64));
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block,
        blocks::bitwise::BitwiseNot,
    };

    #[tokio::test]
    async fn test_and_op() {
        let mut block = BitwiseNot::new();

        write_block_inputs(&mut [(&mut block.input, 2.into())]).await;
        block.execute().await;

        assert_eq!(block.out.value, (-3).into());
    }
}
