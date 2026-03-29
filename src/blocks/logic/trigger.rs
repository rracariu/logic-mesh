// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Detects a change on the Bool input and outputs a True pulse for one cycle.
/// Operation modes: "RisingEdge", "FallingEdge", "RisingOrFallingEdge", "Off".
/// Defaults to "RisingEdge" if not specified.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Trigger {
    #[input(name = "in", kind = "Bool")]
    pub input: InputImpl,
    #[input(kind = "Str")]
    pub operation: InputImpl,
    #[output(kind = "Bool")]
    pub out: OutputImpl,
    prev: Option<bool>,
}

impl Block for Trigger {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let op = match self.operation.get_value() {
            Some(Value::Str(s)) => s.value.as_str().into(),
            _ => Op::RisingEdge,
        };

        if matches!(op, Op::Off) {
            self.out.set(Bool { value: false }.into());
            return;
        }

        let input = self.input.get_value();

        if !matches!(input, Some(Value::Bool(_))) {
            self.out.set(Value::Null);
            return;
        }

        let current = matches!(input, Some(Value::Bool(b)) if b.value);

        let triggered = match self.prev {
            None => false,
            Some(prev) if prev == current => false,
            Some(prev) => match op {
                Op::RisingEdge => !prev && current,
                Op::FallingEdge => prev && !current,
                Op::RisingOrFallingEdge => true,
                Op::Off => false,
            },
        };

        self.prev = Some(current);
        self.out.set(Bool { value: triggered }.into());
    }
}

enum Op {
    RisingEdge,
    FallingEdge,
    RisingOrFallingEdge,
    Off,
}

impl From<&str> for Op {
    fn from(s: &str) -> Self {
        match s {
            "FallingEdge" => Op::FallingEdge,
            "RisingOrFallingEdge" => Op::RisingOrFallingEdge,
            "Off" => Op::Off,
            _ => Op::RisingEdge,
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::logic::Trigger,
    };

    #[tokio::test]
    async fn test_trigger_first_connect_no_pulse() {
        let mut block = Trigger::new();

        // First connection: no previous state, output is false
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }

        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_trigger_rising_edge() {
        let mut block = Trigger::new();

        // First cycle: establish state
        for _ in write_block_inputs(&mut [(&mut block.input, false.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());

        // Rising edge: false → true
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_trigger_pulse_resets() {
        let mut block = Trigger::new();

        // Establish state
        for _ in write_block_inputs(&mut [(&mut block.input, false.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;

        // Rising edge
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Same input, pulse resets
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_trigger_falling_edge() {
        let mut block = Trigger::new();

        // Establish state at true
        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.operation, "FallingEdge".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());

        // Falling edge: true → false
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.operation, "FallingEdge".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_trigger_rising_edge_ignores_falling() {
        let mut block = Trigger::new();

        // Establish state at true
        for _ in write_block_inputs(&mut [(&mut block.input, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;

        // Falling edge with RisingEdge mode: no pulse
        for _ in write_block_inputs(&mut [(&mut block.input, false.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }

    #[tokio::test]
    async fn test_trigger_rising_or_falling() {
        let mut block = Trigger::new();

        // Establish state
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.operation, "RisingOrFallingEdge".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;

        // Rising edge
        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.operation, "RisingOrFallingEdge".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());

        // Falling edge
        for _ in write_block_inputs(&mut [
            (&mut block.input, false.into()),
            (&mut block.operation, "RisingOrFallingEdge".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, true.into());
    }

    #[tokio::test]
    async fn test_trigger_off() {
        let mut block = Trigger::new();

        for _ in write_block_inputs(&mut [
            (&mut block.input, true.into()),
            (&mut block.operation, "Off".into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, false.into());
    }
}
