// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the keys of a dictionary.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
pub struct Keys {
    #[input(name = "input", kind = "Dict")]
    pub input: InputImpl,
    #[output(kind = "List")]
    pub out: OutputImpl,
}

impl Block for Keys {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        match self.input.get_value() {
            Some(Value::Dict(dict)) => {
                self.out.set(
                    dict.keys()
                        .map(|k| k.as_str().into())
                        .collect::<Vec<_>>()
                        .into(),
                )
            }
            _ => self.out.set(Vec::default().into()),
        }
    }
}

#[cfg(test)]
mod test {
    use libhaystack::dict;
    use libhaystack::val::Dict;

    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, blocks::collections::Keys,
    };

    #[tokio::test]
    async fn test_length_block() {
        let mut block = Keys::new();

        write_block_inputs(&mut [(&mut block.input, (Dict::default()).into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, vec![].into());

        write_block_inputs(&mut [(&mut block.input, (dict! {"a" => 1.into()}).into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, vec!["a".into()].into());
    }
}
