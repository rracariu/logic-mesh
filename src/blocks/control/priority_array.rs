// Copyright (c) 2022-2024, Radu Racariu.

use uuid::Uuid;

use crate::base::{
    block::{Block, BlockDesc, BlockProps, BlockState},
    input::{Input, InputProps, input_reader::InputReader},
    output::Output,
};
use libhaystack::val::kind::HaystackKind;

use crate::{blocks::InputImpl, blocks::OutputImpl};

/// Outputs the result of a priority array based on the input values.
#[block]
#[derive(BlockProps, Debug)]
#[category = "control"]
pub struct PriorityArray {
    #[input(name = "ManualLifeSafety", kind = "Number")]
    pub manual_life_safety: InputImpl,
    #[input(name = "AutoLifeSafety", kind = "Number")]
    pub auto_life_safety: InputImpl,
    #[input(kind = "Number")]
    pub priority3: InputImpl,
    #[input(kind = "Number")]
    pub priority4: InputImpl,
    #[input(name = "CriticalEquipmentControl", kind = "Number")]
    pub critical_equipment_control: InputImpl,
    #[input(name = "MinOnOf", kind = "Number")]
    pub min_on_of: InputImpl,
    #[input(kind = "Number")]
    pub priority7: InputImpl,
    #[input(name = "ManualOperator", kind = "Number")]
    pub manual_operator: InputImpl,
    #[input(kind = "Number")]
    pub priority9: InputImpl,
    #[input(kind = "Number")]
    pub priority10: InputImpl,
    #[input(kind = "Number")]
    pub priority11: InputImpl,
    #[input(kind = "Number")]
    pub priority12: InputImpl,
    #[input(kind = "Number")]
    pub priority13: InputImpl,
    #[input(kind = "Number")]
    pub priority14: InputImpl,
    #[input(kind = "Number")]
    pub priority15: InputImpl,
    #[input(kind = "Number")]
    pub priority16: InputImpl,
    #[input(kind = "Number")]
    pub default: InputImpl,

    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for PriorityArray {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(Some(input)) = self
            .inputs()
            .iter()
            .map(|input| input.get_value())
            .find(|input| input.is_some())
        {
            self.out.set(input.clone());
        } else {
            self.out
                .set(self.default.get_value().cloned().unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod test {

    use crate::{
        base::block::Block, base::block::test_utils::write_block_inputs,
        blocks::control::PriorityArray,
    };

    #[tokio::test]
    async fn test_priority_array_block() {
        let mut block = PriorityArray::new();

        write_block_inputs(&mut [(&mut block.manual_life_safety, 100.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 100.into());
    }

    #[tokio::test]
    async fn test_priority_array_min() {
        let mut block = PriorityArray::new();

        write_block_inputs(&mut [
            (&mut block.manual_life_safety, 100.into()),
            (&mut block.manual_operator, 200.into()),
        ])
        .await;

        block.execute().await;
        assert_eq!(block.out.value, 100.into());
    }

    #[tokio::test]
    async fn test_priority_array_max() {
        let mut block = PriorityArray::new();

        write_block_inputs(&mut [(&mut block.priority16, 200.into())]).await;

        block.execute().await;
        assert_eq!(block.out.value, 200.into());
    }
}
