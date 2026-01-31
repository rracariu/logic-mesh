// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs true if the input is not null.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct HasValue {
    #[input(name = "in", kind = "Null")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for HasValue {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let has_value = self.input.get_value().is_some_and(|val| val.has_value());
        self.out.set(has_value.into());
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::misc::has_value::HasValue,
    };

    #[tokio::test]
    async fn has_value_true() {
        let mut block = HasValue::new();

        write_block_inputs(&mut [(&mut block.input, ("test").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn has_value_false() {
        let mut block = HasValue::new();

        write_block_inputs(&mut [(&mut block.input, Value::Null)]).await;

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
