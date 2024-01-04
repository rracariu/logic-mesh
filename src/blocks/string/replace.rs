// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Str, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs a new string based on input string, the needle and the replace value.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Replace"]
#[category = "string"]
pub struct Replace {
    #[input(name = "in", kind = "Str")]
    pub input: InputImpl,
    #[input(kind = "Str")]
    pub find: InputImpl,
    #[input(kind = "Str")]
    pub replace: InputImpl,
    #[output(kind = "Str")]
    pub out: OutputImpl,
}

impl Block for Replace {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Str(input)), Some(Value::Str(find)), Some(Value::Str(replace))) = (
            self.input.get_value(),
            self.find.get_value(),
            self.replace.get_value(),
        ) {
            self.out.set(
                Str {
                    value: input
                        .value
                        .replace(find.value.as_str(), replace.value.as_str()),
                }
                .into(),
            );
        }
    }
}

#[cfg(test)]
mod test {

    use std::assert_matches::assert_matches;

    use libhaystack::val::{Str, Value};

    use crate::{
        base::block::Block,
        base::{
            block::{test_utils::write_block_inputs, BlockProps},
            input::input_reader::InputReader,
        },
        blocks::string::Replace,
    };

    #[tokio::test]
    async fn test_sub() {
        let mut block = Replace::new();

        println!("block: {:?}", block.desc());

        for _ in write_block_inputs(&mut [
            (&mut block.input, "ana are mere".into()),
            (&mut block.find, "ana".into()),
            (&mut block.replace, "ile".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Str(Str { value, .. }) if value == "ile are mere"
        );
    }
}
