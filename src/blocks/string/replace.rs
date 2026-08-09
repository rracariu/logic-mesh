// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::{
    block::Block,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Str, Value};

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
    use assert_matches::assert_matches;

    use libhaystack::val::{Str, Value};

    use crate::{
        base::block::Block,
        base::block::{BlockProps, test_utils::write_block_inputs},
        blocks::string::Replace,
    };

    #[tokio::test]
    async fn test_replace() {
        let mut block = Replace::new();

        println!("block: {:?}", block.desc());

        write_block_inputs([
            (&mut block.input, "ana are mere"),
            (&mut block.find, "ana"),
            (&mut block.replace, "ile"),
        ])
        .await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Str(Str { value, .. }) if value == "ile are mere"
        );
    }
}
