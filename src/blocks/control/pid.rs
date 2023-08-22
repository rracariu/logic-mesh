// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::blocks::utils::input_as_number;
use crate::blocks::utils::to_millis;
use crate::blocks::utils::DEFAULT_SLEEP_DUR;
use crate::{
    base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{input_reader::InputReader, Input, InputProps},
        output::Output,
    },
    blocks::utils::input_as_float_or_default,
};
use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the PID loop result based on sp, KP, KI, and KD inputs.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Pid {
    #[input(kind = "Number")]
    pub sp: InputImpl,
    #[input(kind = "Number")]
    pub kp: InputImpl,
    #[input(kind = "Number")]
    pub ki: InputImpl,
    #[input(kind = "Number")]
    pub kd: InputImpl,
    #[input(kind = "Number")]
    pub interval: InputImpl,
    #[input(name = "min", kind = "Number")]
    pub min: InputImpl,
    #[input(name = "max", kind = "Number")]
    pub max: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,

    integral: f64,
    derivative: f64,
    last_error: f64,
    last_value: f64,
}

impl Block for Pid {
    async fn execute(&mut self) {
        let millis = to_millis(&self.interval.val).unwrap_or(DEFAULT_SLEEP_DUR);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let kp = input_as_number(&self.kp).map(|v| v.value).unwrap_or(1.0);
        let ki = input_as_number(&self.ki).map(|v| v.value).unwrap_or(1.0);
        let kd = input_as_number(&self.kd).map(|v| v.value).unwrap_or(1.0);

        if kp < 0.0 || ki < 0.0 || kd < 0.0 {
            return;
        }

        let sp = input_as_float_or_default(&self.sp);
        let min = input_as_number(&self.min).map(|v| v.value).unwrap_or(0.0);
        let max = input_as_number(&self.max).map(|v| v.value).unwrap_or(100.0);
        let time = millis as f64;

        let cur_value = TryInto::<f64>::try_into(self.out.value()).unwrap_or(0.0);

        // Error
        let error = sp - cur_value;

        // Proportional term
        let proportional = kp * error;

        // Integral term
        self.integral = self.integral + ki * time * (error + self.last_error);

        if self.integral > max / 2.0 {
            self.integral = max / 2.0;
        } else if self.integral < min / 2.0 {
            self.integral = min / 2.0;
        }

        // Derivative term
        self.derivative = -(kd * (cur_value - self.last_value) + time * self.derivative) / time;

        let mut output = proportional + self.integral + self.derivative;

        if output > max {
            output = max;
        } else if output < min {
            output = min;
        }

        self.last_value = output;
        self.last_error = error;

        self.out.set(output.into());
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::test_utils::write_block_inputs,
        base::{block::Block, input::input_reader::InputReader, link::BaseLink},
        blocks::control::Pid,
    };

    #[tokio::test]
    async fn test_pid_block() {
        let mut block = Pid::new();

        for _ in write_block_inputs(&mut [
            (&mut block.interval, (1).into()),
            (&mut block.sp, (100).into()),
        ])
        .await
        {
            block.read_inputs().await;
        }

        block
            .out
            .links
            .push(BaseLink::new(uuid::Uuid::new_v4(), "invalid".to_string()));

        block.execute().await;
        block.execute().await;
        block.execute().await;
        block.execute().await;

        assert_eq!(block.out.value, 50.into());
    }
}
