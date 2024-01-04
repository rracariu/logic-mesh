// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::{
    encoding::zinc,
    val::{kind::HaystackKind, Value},
};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the parsed boolean value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct ParseBool {
    #[input(name = "in", kind = "Str")]
    pub input: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for ParseBool {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Str(input)) = self.input.get_value() {
            let parsed = input.value.parse::<bool>();
            if let Ok(parsed) = parsed {
                self.out.set(parsed.into());
            } else {
                if let Ok(Value::Bool(bool)) = zinc::decode::from_str(&input.value) {
                    self.out.set(bool.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::misc::ParseBool,
    };

    #[tokio::test]
    async fn parse_true() {
        let mut block = ParseBool::new();

        write_block_inputs(&mut [(&mut block.input, ("true").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());

        write_block_inputs(&mut [(&mut block.input, ("T").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn parse_false() {
        let mut block = ParseBool::new();

        write_block_inputs(&mut [(&mut block.input, ("false").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, false.into());

        write_block_inputs(&mut [(&mut block.input, ("F").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
