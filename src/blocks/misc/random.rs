// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use rand::Rng;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::Output,
    },
    blocks::utils::{input_as_number, input_to_millis_or_default},
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::blocks::{InputImpl, OutputImpl};

/// Generates a random number at the specified frequency.
/// min and max control the range of the generated random number.
/// The defaults are 0 and 100.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Random"]
#[category = "misc"]
pub struct Random {
    #[input(kind = "Number")]
    pub freq: InputImpl,
    #[input(kind = "Number")]
    pub min: InputImpl,
    #[input(kind = "Number")]
    pub max: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Random {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.freq.val);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let mut rng = rand::thread_rng();

        let min = input_as_number(&self.min)
            .map(|v| v.value as i64)
            .unwrap_or(0);
        let max = input_as_number(&self.max)
            .map(|v| v.value as i64)
            .unwrap_or(100);

        if min > max {
            self.set_state(BlockState::Fault);
        } else if self.state() == BlockState::Fault {
            self.set_state(BlockState::Running);
        }

        let res = rng.gen_range(min..max);

        self.out.set(Value::make_int(res));
    }
}
