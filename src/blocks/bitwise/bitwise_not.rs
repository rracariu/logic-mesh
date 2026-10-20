// Copyright (c) 2022-2024, Radu Racariu.

//! Bitwise NOT block.

use crate::base::{
    block::Block,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};
use libhaystack::val::Value;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs bitwise NOT operation.
#[block]
#[derive(BlockProps, Debug)]
#[category = "bitwise"]
pub struct BitwiseNot {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for BitwiseNot {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Number(n)) = self.input.value() {
            self.out.set(Value::make_int(!(n.value as i64)));
        }
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::kind::HaystackKind;

    use crate::{
        base::{
            Status,
            block::{Block, connect::connect_output, test_utils::write_block_inputs},
            input::Input,
        },
        blocks::{InputImpl, bitwise::BitwiseNot},
    };

    #[tokio::test]
    async fn test_not_op() {
        let mut block = BitwiseNot::new();

        write_block_inputs([(&mut block.input, 2)]).await;
        block.execute().await;

        assert_eq!(block.out.value, (-3).into());
    }

    /// Regression: the result was written to the output's value field,
    /// bypassing `set`, so it never reached the inputs linked to `out`.
    #[tokio::test]
    async fn result_reaches_linked_inputs() {
        let mut block = BitwiseNot::new();
        let mut target = InputImpl::new("in", HaystackKind::Number, uuid::Uuid::new_v4());
        connect_output(&mut block.out, &mut target).expect("not linked yet");

        write_block_inputs([(&mut block.input, 2)]).await;
        block.execute().await;

        assert_eq!(target.try_take(), Some(((-3).into(), Status::Ok)));
    }
}
