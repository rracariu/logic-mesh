// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Gets the element specified at key from the input and outputs the
/// element's value.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
pub struct GetElement {
    #[input(name = "input", kind = "Null")]
    pub input: InputImpl,
    #[input(name = "key", kind = "Null")]
    pub key: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
}

impl Block for GetElement {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = match (self.input.get_value(), self.key.get_value()) {
            (Some(Value::Dict(dict)), Some(Value::Str(key))) => dict.get(key.as_str()),
            (Some(Value::List(list)), Some(Value::Number(index))) => list.get(index.value as usize),
            _ => None,
        } {
            self.out.set(value.clone());
        } else {
            self.out.set(Value::Null);
        }
    }
}

#[cfg(test)]
mod test {
    use libhaystack::val::Dict;
    use libhaystack::{dict, val::Value};

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::collections::GetElement,
    };

    #[tokio::test]
    async fn test_get_element_block() {
        let mut block = GetElement::new();

        write_block_inputs([
            (&mut block.input, Value::Dict(dict! {"a" => 1})),
            (&mut block.key, "a".into()),
        ])
        .await;

        block.execute().await;
        assert_eq!(block.out.value, 1.into());

        write_block_inputs([(&mut block.key, "x")]).await;
        block.execute().await;
        assert_eq!(block.out.value, Value::Null);

        write_block_inputs([(&mut block.input, ""), (&mut block.key, "a")]).await;

        block.execute().await;
        assert_eq!(block.out.value, Value::Null);

        write_block_inputs([
            (&mut block.input, Value::List(vec![1.into(), 2.into()])),
            (&mut block.key, 1.into()),
        ])
        .await;

        block.execute().await;
        assert_eq!(block.out.value, 2.into());

        write_block_inputs([(&mut block.key, 100)]).await;
        block.execute().await;
        assert_eq!(block.out.value, Value::Null);
    }
}
