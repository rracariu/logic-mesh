// Copyright (c) 2022-2023, Radu Racariu.

use std::time::Duration;

use uuid::Uuid;

use crate::base::output::props::OutputProps;
use crate::blocks::utils::input_as_number;
use crate::blocks::utils::input_to_millis_or_default;
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

/// Outputs the PID loop result based on input, sp, KP, KI, and KD inputs.
/// If input is not connected, the output value is used for the input.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct Pid {
    #[input(kind = "Number")]
    pub input: InputImpl,

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

    #[input(kind = "Number")]
    pub min: InputImpl,

    #[input(kind = "Number")]
    pub max: InputImpl,

    #[input(kind = "Number")]
    pub bias: InputImpl,

    #[output(kind = "Number")]
    pub out: OutputImpl,

    integral: f64,
    derivative: f64,
    last_error: f64,
    last_value: f64,
}

impl Block for Pid {
    async fn execute(&mut self) {
        let millis = input_to_millis_or_default(&self.interval.val);

        self.wait_on_inputs(Duration::from_millis(millis)).await;

        if !self.out.is_connected() {
            return;
        }

        let kp = input_as_number(&self.kp).map(|v| v.value).unwrap_or(0.98);
        let ki = input_as_number(&self.ki).map(|v| v.value).unwrap_or(0.002);
        let kd = input_as_number(&self.kd).map(|v| v.value).unwrap_or(0.25);
        let bias = input_as_number(&self.kd).map(|v| v.value).unwrap_or(100.0);

        if kp <= 0.0 || ki <= 0.0 || kd <= 0.0 {
            return;
        }

        let sp = input_as_float_or_default(&self.sp);
        let min = input_as_number(&self.min).map(|v| v.value).unwrap_or(0.0);
        let max = input_as_number(&self.max).map(|v| v.value).unwrap_or(100.0);
        let time = millis as f64;

        let cur_value = if self.input.is_connected() {
            input_as_number(&self.input).map(|v| v.value).unwrap_or(0.0)
        } else {
            TryInto::<f64>::try_into(self.out.value()).unwrap_or(0.0)
        };

        // Error
        let error = sp - cur_value;

        // Proportional term
        let proportional = kp * error;

        // Integral term
        self.integral += ki * time * (error + self.last_error);
        self.integral = self.integral.clamp(min, max);

        // Derivative term
        self.derivative = -(bias * kd * (cur_value - self.last_value)
            + (bias - time) * self.derivative)
            / (bias + time);

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
            (&mut block.kp, (1.0).into()),
            (&mut block.ki, (1.0).into()),
            (&mut block.kd, (1.0).into()),
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

        assert_eq!(block.out.value, 100.into());
    }
}
