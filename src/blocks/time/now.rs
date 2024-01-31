// Copyright (c) 2022-2024, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::{Output, OutputProps},
    },
    blocks::utils::{to_millis, DEFAULT_SLEEP_DUR},
};

use crate::{blocks::InputImpl, blocks::OutputImpl};
use libhaystack::val::kind::HaystackKind;
use std::time::{SystemTime, UNIX_EPOCH};

/// Outputs the current wall clock time in millis at the desired resolution.
#[block]
#[derive(BlockProps, Debug)]
#[category = "logic"]
pub struct Now {
    #[input(name = "resolution", kind = "Number")]
    pub resolution: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Now {
    async fn execute(&mut self) {
        let millis = to_millis(&self.resolution.val).unwrap_or(DEFAULT_SLEEP_DUR);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let now = SystemTime::now();
        match now.duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                self.out.set((duration.as_millis() as f64).into());
                self.set_state(BlockState::Running);
            }
            Err(_) => {
                self.set_state(BlockState::Fault);
            }
        };
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::{block::Block, link::BaseLink, output::Output},
        blocks::time::Now,
    };

    #[tokio::test]
    async fn test_and_block() {
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
