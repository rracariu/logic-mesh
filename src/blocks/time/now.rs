// Copyright (c) 2022-2024, Radu Racariu.

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::{Output, OutputProps},
    },
    blocks::utils::input_to_millis_or_default,
};
use std::time::Duration;
use uuid::Uuid;

use crate::tokio_impl::sleep::current_time_millis;
use crate::{blocks::InputImpl, blocks::OutputImpl};
use libhaystack::val::kind::HaystackKind;

/// Outputs the current wall clock time in millis at the desired resolution.
#[block]
#[derive(BlockProps, Debug)]
#[category = "time"]
pub struct Now {
    #[input(name = "resolution", kind = "Number")]
    pub resolution: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Now {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.resolution.val);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        self.out.set((current_time_millis() as f64).into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::{block::Block, link::BaseLink, output::Output},
        blocks::time::Now,
    };

    #[tokio::test]
    async fn test_now_block() {
        let mut block = Now::new();
        block
            .out
            .add_link(BaseLink::new(uuid::Uuid::new_v4(), "testIn".into()));

        block.execute().await;
        let now = block.out.value.clone();

        block.execute().await;
        assert!(block.out.value > now);
    }
}
