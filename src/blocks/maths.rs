// Copyright (c) 2022-2023, IntriSemantics Corp.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps},
    output::Output,
};

use libhaystack::val::{kind::HaystackKind, Number, Value};

use super::{read_block_inputs, InputImpl, OutputImpl};

#[block]
#[derive(BlockProps, Debug)]
#[name = "Add"]
#[library = "math"]
#[input(kind = "Number", count = 16)]
pub struct Add {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Add {
    async fn execute(&mut self) {
        read_block_inputs(self).await;

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
            println!("Add: {}", res.value);
            self.out.set((res).into()).await
        }
    }
}
