// Copyright (c) 2022-2026, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::input_as_number;

use libhaystack::val::{Bool, Value, kind::HaystackKind};

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Two-device lead-lag controller. `demand` is the number of devices
/// that should run (0, 1, or 2). The "lead" device is whichever one
/// currently holds the lead role; the other is "lag". A rising edge on
/// `rotate` swaps the roles. When `enable` is false both outputs are
/// off.
///
/// Pair with [`Sequencer`](super::Sequencer) to drive `demand` from a
/// continuous load signal, or with [`Schedule`](crate::blocks::time::Schedule)
/// + a weekly cron to rotate at a fixed time.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct LeadLag {
    #[input(kind = "Bool")]
    pub enable: InputImpl,
    #[input(kind = "Bool")]
    pub rotate: InputImpl,
    #[input(kind = "Number")]
    pub demand: InputImpl,
    #[output(name = "a", kind = "Bool")]
    pub out_a: OutputImpl,
    #[output(name = "b", kind = "Bool")]
    pub out_b: OutputImpl,
    lead_index: u8,
    prev_rotate: Option<bool>,
}

impl Block for LeadLag {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        let enable = matches!(self.enable.get_value(), Some(Value::Bool(b)) if b.value);
        let rotate = matches!(self.rotate.get_value(), Some(Value::Bool(b)) if b.value);
        let demand = input_as_number(&self.demand)
            .map(|n| n.value as i32)
            .unwrap_or(0)
            .max(0);

        let rising_rotate = matches!(self.prev_rotate, Some(false)) && rotate;
        self.prev_rotate = Some(rotate);
        if rising_rotate {
            self.lead_index ^= 1;
        }

        let (a_on, b_on) = if !enable {
            (false, false)
        } else {
            let lead_on = demand >= 1;
            let lag_on = demand >= 2;
            if self.lead_index == 0 {
                (lead_on, lag_on)
            } else {
                (lag_on, lead_on)
            }
        };

        self.out_a.set(Bool { value: a_on }.into());
        self.out_b.set(Bool { value: b_on }.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader},
        blocks::control::LeadLag,
    };

    #[tokio::test]
    async fn test_leadlag_disable() {
        let mut block = LeadLag::new();
        for _ in write_block_inputs(&mut [
            (&mut block.enable, false.into()),
            (&mut block.rotate, false.into()),
            (&mut block.demand, (2.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out_a.value, false.into());
        assert_eq!(block.out_b.value, false.into());
    }

    #[tokio::test]
    async fn test_leadlag_demand_one_lead_only() {
        let mut block = LeadLag::new();
        for _ in write_block_inputs(&mut [
            (&mut block.enable, true.into()),
            (&mut block.rotate, false.into()),
            (&mut block.demand, (1.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out_a.value, true.into());
        assert_eq!(block.out_b.value, false.into());
    }

    #[tokio::test]
    async fn test_leadlag_demand_two_both_on() {
        let mut block = LeadLag::new();
        for _ in write_block_inputs(&mut [
            (&mut block.enable, true.into()),
            (&mut block.rotate, false.into()),
            (&mut block.demand, (2.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out_a.value, true.into());
        assert_eq!(block.out_b.value, true.into());
    }

    #[tokio::test]
    async fn test_leadlag_rotate_swaps_lead() {
        let mut block = LeadLag::new();

        // Lead is A initially with demand=1
        for _ in write_block_inputs(&mut [
            (&mut block.enable, true.into()),
            (&mut block.rotate, false.into()),
            (&mut block.demand, (1.0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out_a.value, true.into());
        assert_eq!(block.out_b.value, false.into());

        // Rising edge on rotate → B becomes lead
        for _ in write_block_inputs(&mut [(&mut block.rotate, true.into())]).await {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out_a.value, false.into());
        assert_eq!(block.out_b.value, true.into());
    }
}
