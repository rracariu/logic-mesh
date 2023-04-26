// Copyright (c) 2022-2023, IntriSemantics Corp.

use futures::{future::select_all, FutureExt};
use rand::Rng;

use uuid::Uuid;

use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps},
        output::Output,
    },
    tokio_impl::block::read_block_inputs_no_index,
};

use libhaystack::val::{kind::HaystackKind, Value};
use libhaystack::{units::units_generated::MILLISECOND, val::Number};

use super::{
    utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    InputImpl, OutputImpl,
};

/// Block that generates a sine wave based on
/// the frequency and the amplitude inputs.
#[block]
#[derive(BlockProps, Debug)]
#[name = "SineWave"]
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
            read_block_inputs_no_index(self).boxed_local(),
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
            read_block_inputs_no_index(self).boxed_local(),
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

fn input_as_float_or_default(input: &InputImpl) -> f64 {
    input_as_number(input).map(|v| v.value).unwrap_or(0.0)
}

fn input_as_number(input: &InputImpl) -> Option<Number> {
    if let Some(Value::Number(val)) = input.val {
        Some(val)
    } else {
        None
    }
}

fn to_millis(dur: &Option<Value>) -> Result<u64, ()> {
    if let Some(Value::Number(dur)) = dur {
        if let Some(unit) = dur.unit {
            match unit.convert_to(dur.value, &MILLISECOND) {
                Ok(millis) => Ok(millis as u64),
                Err(_) => Err(()),
            }
        } else {
            Ok(dur.value as u64)
        }
    } else {
        Err(())
    }
}
