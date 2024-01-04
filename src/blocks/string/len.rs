// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the length of the input string.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "StringLen"]
#[category = "string"]
pub struct StrLen {
    #[input(name = "in", kind = "Str")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for StrLen {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Str(a)) = self.input.get_value() {
            self.out.set(
                Number {
                    value: a.value.len() as f64,
                    unit: None,
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {

    use std::assert_matches::assert_matches;

    use libhaystack::val::{Number, Value};

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::string::StrLen,
    };

    #[tokio::test]
    async fn test_sub() {
        let mut block = StrLen::new();

        write_block_inputs(&mut [(&mut block.input, "ana are mere".into())]).await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Number(Number { value, .. }) if value == 12.0
        );
    }
}
