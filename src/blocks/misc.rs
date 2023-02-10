// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::time::Duration;

use tokio::time::sleep;
use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Value};
use libhaystack::{units::units_generated::MILLISECOND, val::Number};

use super::{read_block_inputs, InputImpl, OutputImpl};

#[block]
#[derive(BlockProps, Debug)]
#[name = "SineWave"]
#[library = "math"]
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
        read_block_inputs(self).await;

        if let Ok(millis) = self.freq_millis() {
            let amp = input_as_float_or_default(&self.amplitude);
            let amp = if amp == 0.0 { 1.0 } else { amp };

            let res = amp * (self.count / millis as f64).sin();

            sleep(Duration::from_millis(millis)).await;
            self.count += 1.0;
            self.out.set(res.into()).await;
        } else {
            self.set_state(BlockState::Fault);
        }
    }
}

impl SineWave {
    fn freq_millis(&self) -> Result<u64, ()> {
        if let Some(Value::Number(dur)) = self.freq.get_value() {
            to_millis(dur)
        } else {
            Err(())
        }
    }
}

fn input_as_float_or_default(input: &InputImpl) -> f64 {
    input
        .get_value()
        .as_ref()
        .and_then(|v| match v {
            Value::Number(v) => Some(v.value),
            _ => None,
        })
        .unwrap_or_default()
}

fn input_as_int_or_default(input: &InputImpl) -> i64 {
    input_as_float_or_default(input) as i64
}

fn to_millis(dur: &Number) -> Result<u64, ()> {
    if let Some(unit) = dur.unit {
        match unit.convert_to(dur.value, &MILLISECOND) {
            Ok(millis) => Ok(millis as u64),
            Err(_) => Err(()),
        }
    } else {
        Ok(dur.value as u64)
    }
}
