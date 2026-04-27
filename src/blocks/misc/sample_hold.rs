// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};

use libhaystack::val::{Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Sample-and-hold. On a rising edge of `trigger`, the current value of
/// `in` is captured and emitted on `out`. The output then holds that
/// value until the next rising edge. Until the first trigger, the output
/// is null.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct SampleHold {
    #[input(name = "in", kind = "Null")]
    pub input: InputImpl,
    #[input(kind = "Bool")]
    pub trigger: InputImpl,
    #[output(kind = "Null")]
    pub out: OutputImpl,
    prev_trigger: Option<bool>,
}

impl Block for SampleHold {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let trig = matches!(self.trigger.get_value(), Some(Value::Bool(b)) if b.value);
        let rising = matches!(self.prev_trigger, Some(false)) && trig;
        self.prev_trigger = Some(trig);

        if rising && let Some(v) = self.input.get_value() {
            self.out.set(v.clone());
        }
    }
}

#[cfg(test)]
mod test {

    use libhaystack::val::Value;

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::misc::SampleHold,
    };

    #[tokio::test]
    async fn test_sample_hold_captures_on_rising_edge() {
        let mut block = SampleHold::new();

        // Establish trigger=false
        for _ in write_block_inputs(&mut [
            (&mut block.input, (10.0).into()),
            (&mut block.trigger, false.into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, Value::Null);

        // Rising edge → captures 10
        for _ in write_block_inputs(&mut [(&mut block.trigger, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (10.0).into());

        // Input changes but trigger held high → still 10
        for _ in write_block_inputs(&mut [(&mut block.input, (99.0).into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (10.0).into());

        // Trigger drops, then rises again → captures 99
        for _ in write_block_inputs(&mut [(&mut block.trigger, false.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        for _ in write_block_inputs(&mut [(&mut block.trigger, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (99.0).into());
    }
}
