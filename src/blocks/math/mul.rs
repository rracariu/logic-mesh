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

/// Performs a multiplication of multiple numbers from the 16 inputs
/// this block has.
/// The operation would take into account the units of those input's values,
/// if the units are not convertible, the block would be in an error state.
#[block]
#[derive(BlockProps, Debug)]
#[category = "math"]
#[input(kind = "Number", count = 16)]
pub struct Mul {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Mul {
    async fn execute(&mut self) {
        let input = self.read_inputs().await;

        if input.is_none() {
            sleep_millis(DEFAULT_SLEEP_DUR).await;
            return;
        }

        let mut val: Option<Number> = None;
        let mut cnt = 0;
        for el in self
            .inputs()
            .into_iter()
            .filter_map(|input| match input.get_value().as_ref() {
                Some(Value::Number(num)) => Some(*num),
                _ => None,
            })
        {
            cnt += 1;

            if let Some(v) = val {
                let res = v * el;

                if res.is_err() {
                    val = None;
                    break;
                }

                match res {
                    Ok(res) => {
                        val.replace(res);
                    }
                    Err(_) => {
                        val = None;
                        break;
                    }
                }
            } else {
                val = Some(el);
            }
        }

        if cnt > 1 {
            if let Some(res) = val {
                self.out.set(res.into())
            }
        }
    }
}

#[cfg(test)]
mod test {

    use crate::base::input::input_reader::InputReader;
    use crate::{
        base::block::{Block, BlockProps},
        blocks::math::Mul,
    };

    #[tokio::test]
    async fn test_mul_block() {
        let mut block = Mul::new();

        {
            let in1 = block.get_input_mut("in0").unwrap();
            in1.increment_conn();
            in1.writer().try_send(3.into()).unwrap();
            block.read_inputs().await;
        }

        {
            let in16 = block.get_input_mut("in15").unwrap();
            in16.increment_conn();
            in16.writer().try_send(3.into()).unwrap();
        }

        block.execute().await;
        assert_eq!(block.out.value, 9.into());
    }
}
