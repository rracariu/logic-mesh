// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::Output,
    },
    blocks::utils::{input_as_float_or_default, to_millis},
};

use libhaystack::val::kind::HaystackKind;

use crate::{
    blocks::utils::DEFAULT_SLEEP_DUR,
    blocks::{InputImpl, OutputImpl},
};

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
        let millis = to_millis(&self.freq.val).unwrap_or(DEFAULT_SLEEP_DUR);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let millis = to_millis(&self.freq.val).unwrap_or(DEFAULT_SLEEP_DUR);

        let amp = input_as_float_or_default(&self.amplitude);
        let amp = if amp == 0.0 { 1.0 } else { amp };

        let res = amp * (self.count / millis as f64).sin();

        self.count += 1.0;
        self.out.set(res.into());
    }
}
