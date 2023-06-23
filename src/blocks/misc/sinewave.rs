// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::{future::select_all, FutureExt};
use uuid::Uuid;

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
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::{InputImpl, OutputImpl},
};

/// Block that generates a sine wave based on
/// the frequency and the amplitude inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "math"]
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

        let (_, index, _) = select_all([
            sleep_millis(millis).boxed_local(),
            self.wait_on_inputs().boxed_local(),
        ])
        .await;

        let millis = to_millis(&self.freq.val).unwrap_or(DEFAULT_SLEEP_DUR);

        let amp = input_as_float_or_default(&self.amplitude);
        let amp = if amp == 0.0 { 1.0 } else { amp };

        let res = amp * (self.count / millis as f64).sin();

        if index != 0 {
            sleep_millis(millis).await;
        }

        self.count += 1.0;
        self.out.set(res.into());
    }
}
