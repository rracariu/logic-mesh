// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::{future::select_all, FutureExt};
use rand::Rng;

use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::Output,
    },
    blocks::utils::{input_as_number, to_millis},
};

use libhaystack::val::{kind::HaystackKind, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::{InputImpl, OutputImpl},
};

/// Generates a random number at the specified frequency.
/// min and max control the range of the generated random number.
/// The defaults are 0 and 100.
#[block]
#[derive(BlockProps, Debug)]
#[name = "Random"]
#[category = "math"]
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
        let millis = to_millis(&self.freq.val).unwrap_or(DEFAULT_SLEEP_DUR);

        let (_, index, _) = select_all([
            sleep_millis(millis).boxed_local(),
            self.wait_on_inputs().boxed_local(),
        ])
        .await;

        let millis = to_millis(&self.freq.val).unwrap_or(DEFAULT_SLEEP_DUR);
        let mut rng = rand::thread_rng();

        let min = input_as_number(&self.min)
            .map(|v| v.value as i64)
            .unwrap_or(0);
        let max = input_as_number(&self.max)
            .map(|v| v.value as i64)
            .unwrap_or(100);

        let res = rng.gen_range(min..max);

        if index != 0 {
            sleep_millis(millis).await;
        }

        self.out.set(Value::make_int(res));
    }
}
