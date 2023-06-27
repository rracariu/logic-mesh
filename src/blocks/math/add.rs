// Copyright (c) 2022-2023, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{input_reader::InputReader, Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use crate::{
    blocks::utils::{sleep_millis, DEFAULT_SLEEP_DUR},
    blocks::InputImpl,
    blocks::OutputImpl,
};

/// Performs an addition of multiple numbers from the 16 inputs
/// this block has.
/// The addition would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Add"]
#[category = "math"]
#[input(kind = "Number", count = 16)]
pub struct Add {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Add {
    async fn execute(&mut self) {
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        let mut has_err = false;

        let val = self
            .inputs()
            .into_iter()
            .filter_map(|input| match input.get_value().as_ref() {
                Some(Value::Number(num)) => Some(*num),
                _ => None,
            })
            .reduce(|acc, val| {
                let mut acc = acc;

                if acc.unit.is_none() && acc.value == 0.0 {
                    if let Some(unit) = val.unit {
                        acc = Number::make_with_unit(0.0, unit);
                    }
                };

                match acc + val {
                    Ok(res) => res,
                    Err(_) => {
                        has_err = true;
                        Number::make(0.0)
                    }
                }
            });

        if has_err {
            self.set_state(BlockState::Fault);
        } else if self.state() != BlockState::Running {
            self.set_state(BlockState::Running);
        }

        if let Some(res) = val {
            self.out.set((res).into())
        }
    }
}
