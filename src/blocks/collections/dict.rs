// Copyright (c) 2022-2023, Radu Racariu.

//! Dict builder block.

use crate::base::{
    block::Block,
    input::{InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Dict as HDict, Value};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs a dictionary of elements constructed from the input keys and values.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
pub struct Dict {
    #[input(kind = "List")]
    pub keys: InputImpl,
    #[input(kind = "List")]
    pub values: InputImpl,
    #[output(kind = "Dict")]
    pub out: OutputImpl,
}

impl Block for Dict {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::List(keys)), Some(Value::List(values))) =
            (self.keys.value(), self.values.value())
        {
            let mut dict = HDict::new();

            for (key, value) in keys.iter().zip(values.iter()) {
                if let Value::Str(key) = key {
                    dict.insert(key.value().into(), value.clone());
                }
            }

            self.out.set(dict.into())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        base::block::Block, base::block::BlockProps, base::block::test_utils::write_block_inputs,
        blocks::collections::Dict as DictBlock,
    };
    use libhaystack::{dict, val::Dict, val::Value};

    #[tokio::test]
    async fn test_dict_block() {
        let mut block = DictBlock::new();

        write_block_inputs([(block.inputs_mut()[0], vec!["a".into()])]).await;
        write_block_inputs([(block.inputs_mut()[1], vec![200.into()])]).await;

        block.execute().await;
        assert_eq!(block.out.value, Value::Dict(dict! {"a" => 200}));
    }

    /// Regression: an empty `values` list underflowed the bound the keys
    /// were checked against, which panics in a debug build.
    #[tokio::test]
    async fn keys_without_values_make_an_empty_dict() {
        let mut block = DictBlock::new();

        write_block_inputs([(block.inputs_mut()[0], vec!["a".into()])]).await;
        write_block_inputs([(block.inputs_mut()[1], Vec::<Value>::new())]).await;

        block.execute().await;
        assert_eq!(block.out.value, Value::Dict(Dict::default()));
    }
}
