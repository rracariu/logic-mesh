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

/// Outputs the parsed numeric value of the input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct ParseNumber {
    #[input(name = "in", kind = "Str")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for ParseNumber {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Value::Str(input)) = self.input.get_value() {
            let parsed = input.value.parse::<f64>();
            if let Ok(parsed) = parsed {
                self.out.set(parsed.into());
            } else if let Ok(Value::Number(number)) = zinc::decode::from_str(&input.value) {
                self.out.set(number.into());
            }
        }
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::misc::ParseNumber,
    };

    #[tokio::test]
    async fn test_parse_number() {
        let mut block = ParseNumber::new();

        write_block_inputs(&mut [(&mut block.input, ("33.5").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 33.5.into());
    }

    #[tokio::test]
    async fn test_parse_number_unit() {
        let mut block = ParseNumber::new();

        write_block_inputs(&mut [(&mut block.input, ("33.5F").into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, Value::make_number_unit(33.5, "F".into()));
    }
}
