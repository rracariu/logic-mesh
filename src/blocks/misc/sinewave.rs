// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps, input_reader::InputReader},
        output::Output,
    },
    blocks::utils::{input_as_float_or_default, input_to_millis_or_default},
};

use libhaystack::val::kind::HaystackKind;

use crate::blocks::{InputImpl, OutputImpl};

/// Block that generates a sine wave based on
/// the frequency and the amplitude inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "misc"]
pub struct SineWave {
    #[input(kind = "Number")]
    pub freq: InputImpl,
    #[input(kind = "Number")]
    pub amplitude: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    count: f64,
}

impl Block for SineWave {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.freq.val);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let millis = input_to_millis_or_default(&self.freq.val);

        let amp = input_as_float_or_default(&self.amplitude);
        let amp = if amp == 0.0 { 1.0 } else { amp };

        let res = amp * (self.count / millis as f64).sin();

        self.count += 1.0;
        self.out.set(res.into());
    }
}
