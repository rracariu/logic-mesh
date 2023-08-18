// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

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
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        if let Some(Value::Str(input)) = self.input.get_value() {
            let parsed = input.value.parse::<f64>();
            if let Ok(parsed) = parsed {
                self.out.set(parsed.into());
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::misc::ParseNumber,
    };

    #[tokio::test]
    async fn test_parse_number_block() {
        let mut block = ParseNumber::new();

        for _ in write_block_inputs(&mut [(&mut block.input, ("33.5").into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, 33.5.into());
    }
}
