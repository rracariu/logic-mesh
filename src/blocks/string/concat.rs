// Copyright (c) 2022-2024, Radu Racariu.

use crate::base::{
    block::{Block, BlockProps},
    input::input_reader::InputReader,
    output::Output,
};

use libhaystack::val::Value;

use crate::blocks::OutputImpl;

/// Outputs the concatenated value of all the input strings.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Concat"]
#[category = "string"]
#[input(kind = "Str", count = 16)]
pub struct Concat {
    #[output(kind = "Str")]
    pub out: OutputImpl,
}

impl Block for Concat {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let result = self
            .inputs()
            .iter()
            .filter_map(|input| match input.get_value() {
                Some(Value::Str(val)) => Some(val.value.clone()),
                _ => None,
            })
            .reduce(|mut acc, val| {
                acc.push_str(&val);
                acc
            });

        if let Some(result) = result {
            self.out.set(result.as_str().into())
        }
    }
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use libhaystack::val::{Str, Value};

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::block::{Block, BlockProps},
        blocks::string::Concat,
    };

    #[tokio::test]
    async fn test_concat() {
        let mut block = Concat::new();

        write_block_inputs([(block.inputs_mut()[0], "first ")]).await;
        write_block_inputs([(block.inputs_mut()[1], "last")]).await;

        block.execute().await;

        assert_matches!(
            block.out.value,
            Value::Str(Str { value}) if value == "first last"
        );
    }
}
