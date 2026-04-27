// Copyright (c) 2022-2026, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use crate::blocks::utils::{get_sleep_dur, input_as_number, input_to_millis_or_default};
use crate::tokio_impl::sleep::current_time_millis;

use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Equipment stager. Maps a `demand` in `[0..1]` to an integer stage
/// count in `[0..stages]`. Stage-up transitions wait `upDelay` ms with
/// the higher target; stage-down transitions wait `downDelay` ms with
/// the lower target. Use the output to gate boilers, chillers, or
/// compressors and avoid short-cycling.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Sequencer {
    #[input(kind = "Number")]
    pub demand: InputImpl,
    #[input(kind = "Number")]
    pub stages: InputImpl,
    #[input(name = "upDelay", kind = "Number")]
    pub up_delay: InputImpl,
    #[input(name = "downDelay", kind = "Number")]
    pub down_delay: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    current: u32,
    pending_target: i32,
    pending_since_ms: u64,
}

impl Block for Sequencer {
    async fn execute(&mut self) {
        let poll = get_sleep_dur();
        self.wait_on_inputs(Duration::from_millis(poll)).await;

        if !self.out.is_connected() {
            return;
        }

        let demand = input_as_number(&self.demand)
            .map(|n| n.value.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let stages = input_as_number(&self.stages)
            .map(|n| n.value as i32)
            .unwrap_or(1)
            .max(0);
        let up_delay = input_to_millis_or_default(&self.up_delay.val);
        let down_delay = input_to_millis_or_default(&self.down_delay.val);

        let target = (demand * stages as f64).ceil() as i32;
        let target = target.clamp(0, stages);
        let current = self.current as i32;
        let now = current_time_millis();

        if target == current {
            self.pending_target = current;
            self.pending_since_ms = 0;
            self.out.set((current as f64).into());
            return;
        }

        // Different target: arm the timer if it's not already armed for
        // this exact target.
        if self.pending_target != target || self.pending_since_ms == 0 {
            self.pending_target = target;
            self.pending_since_ms = now;
            self.out.set((current as f64).into());
            return;
        }

        let elapsed = now.saturating_sub(self.pending_since_ms);
        let needed = if target > current { up_delay } else { down_delay };

        if elapsed >= needed {
            // Move one step at a time so each step gets re-timed.
            self.current = if target > current {
                (current + 1) as u32
            } else {
                (current - 1) as u32
            };
            self.pending_since_ms = 0;
            self.pending_target = self.current as i32;
        }

        self.out.set((self.current as f64).into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::control::Sequencer,
    };

    fn link_out(block: &mut Sequencer) {
        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "test".to_string()));
    }

    #[tokio::test]
    async fn test_sequencer_zero_demand() {
        let mut block = Sequencer::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.demand, (0.0).into()),
            (&mut block.stages, (4.0).into()),
            (&mut block.up_delay, (0).into()),
            (&mut block.down_delay, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        assert_eq!(block.out.value, (0.0_f64).into());
    }

    #[tokio::test]
    async fn test_sequencer_zero_delay_advances_one_step_per_cycle() {
        let mut block = Sequencer::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.demand, (1.0).into()),
            (&mut block.stages, (4.0).into()),
            (&mut block.up_delay, (0).into()),
            (&mut block.down_delay, (0).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        // First exec arms the timer, second exec advances by 1 stage.
        block.execute().await;
        block.execute().await;
        assert_eq!(block.current, 1);
        block.execute().await;
        block.execute().await;
        assert_eq!(block.current, 2);
    }

    #[tokio::test]
    async fn test_sequencer_long_delay_holds() {
        let mut block = Sequencer::new();
        link_out(&mut block);

        for _ in write_block_inputs(&mut [
            (&mut block.demand, (1.0).into()),
            (&mut block.stages, (4.0).into()),
            (&mut block.up_delay, (3_600_000).into()),
            (&mut block.down_delay, (3_600_000).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }
        block.execute().await;
        block.execute().await;
        assert_eq!(block.current, 0);
    }
}
