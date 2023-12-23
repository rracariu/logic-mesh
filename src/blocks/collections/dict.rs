// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Dict as HDict, Value};

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
            (self.keys.get_value(), self.values.get_value())
        {
            let mut dict = HDict::new();

            for (i, key) in keys.iter().enumerate() {
                if i > values.len() - 1 {
                    break;
                }

                if let (Value::Str(key), Some(value)) = (key, values.get(i)) {
                    dict.insert(key.value.clone(), value.clone());
                }
            }

            self.out.set(dict.into())
        }
    }
}

#[cfg(test)]
mod test {
    use crate::base::input::input_reader::InputReader;
    use crate::{
        base::block::test_utils::write_block_inputs, base::block::Block, base::block::BlockProps,
        blocks::collections::Dict as DictBlock,
    };
    use libhaystack::{dict, val::Dict, val::Value};

    #[tokio::test]
    async fn test_dict_block() {
        let mut block = DictBlock::new();

        write_block_inputs(&mut [(block.inputs_mut()[0], vec!["a".into()].into())]).await;
        write_block_inputs(&mut [(block.inputs_mut()[1], vec![200.into()].into())]).await;
        block.read_inputs().await;

        block.execute().await;
        assert_eq!(
            block.out.value,
            Value::Dict(dict! {"a" => 200.into()}).into()
        );
    }
}
