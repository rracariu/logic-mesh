// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Flip-Flop block. Set input prioritizes over Reset when both are true.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct FlipFlop {
    #[input(name = "set", kind = "Bool")]
    pub set: InputImpl,
    #[input(name = "reset", kind = "Bool")]
    pub reset: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
}

impl Block for FlipFlop {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let (Some(Value::Bool(s)), Some(Value::Bool(r))) =
            (self.set.get_value(), self.reset.get_value())
        {
            let current = matches!(&self.out.value, Value::Bool(b) if b.value);

            let next = if s.value {
                true
            } else if r.value {
                false
            } else {
                current
            };

            self.out.set(Bool { value: next }.into());
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::FlipFlop,
    };

    #[tokio::test]
    async fn test_flip_flop_set() {
        let mut block = FlipFlop::new();

        for _ in write_block_inputs(&mut [
            (&mut block.set, true.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_flip_flop_reset() {
        let mut block = FlipFlop::new();

        for _ in write_block_inputs(&mut [
            (&mut block.set, false.into()),
            (&mut block.reset, true.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_flip_flop_set_priority() {
        let mut block = FlipFlop::new();

        // When both Set and Reset are true, Set takes priority
        for _ in write_block_inputs(&mut [
            (&mut block.set, true.into()),
            (&mut block.reset, true.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_flip_flop_hold_state() {
        let mut block = FlipFlop::new();

        // First set the flip-flop
        for _ in write_block_inputs(&mut [
            (&mut block.set, true.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Both inputs false, should hold state
        for _ in write_block_inputs(&mut [
            (&mut block.set, false.into()),
            (&mut block.reset, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }
}
