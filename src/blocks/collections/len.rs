// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

///  Outputs the number of elements in the the collection.
#[block]
#[derive(BlockProps, Debug)]
#[category = "collections"]
pub struct Length {
    #[input(name = "input", kind = "Null")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Length {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        match self.input.get_value() {
            Some(Value::Dict(dict)) => self.out.set((dict.len() as f64).into()),
            Some(Value::List(list)) => self.out.set((list.len() as f64).into()),
            Some(Value::Str(str)) => self.out.set((str.value.len() as f64).into()),
            _ => self.out.set(0.into()),
        }
    }
}

#[cfg(test)]
mod test {
    use libhaystack::dict;
    use libhaystack::val::Dict;

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::collections::Length,
    };

    #[tokio::test]
    async fn test_length_block() {
        let mut block = Length::new();

        write_block_inputs(&mut [(&mut block.input, (Dict::default()).into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, 0.into());

        write_block_inputs(&mut [(&mut block.input, (dict! {"a" => 1.into()}).into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, 1.into());

        write_block_inputs(&mut [(&mut block.input, vec![1.into()].into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, 1.into());

        write_block_inputs(&mut [(&mut block.input, vec![].into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, 0.into());

        write_block_inputs(&mut [(&mut block.input, "ana are mere".into())]).await;
        block.execute().await;
        assert_eq!(block.out.value, 12.into());
    }
}
