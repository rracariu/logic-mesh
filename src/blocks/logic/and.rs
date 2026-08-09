// Copyright (c) 2022-2023, Radu Racariu.

use crate::base::{
    block::Block,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Bool, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the logical And value of the inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct And {
    #[input(name = "in1", kind = "Bool")]
    pub input1: InputImpl,
    #[input(name = "in2", kind = "Bool")]
    pub input2: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for And {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Bool(a)), Some(Value::Bool(b))) =
            (self.input1.get_value(), self.input2.get_value())
        {
            self.out.set(
                Bool {
                    value: a.value && b.value,
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs, blocks::logic::And,
    };

    #[tokio::test]
    async fn test_and_block() {
        let mut block = And::new();

        write_block_inputs([
            (&mut block.input1, 0.into()),
            (&mut block.input2, Value::make_true()),
        ])
        .await;

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
