use libhaystack::val::Value;

use crate::base::block::{convert_value, BlockProps};

/// A block that has two inputs and one output.
pub trait BinaryBlock: BlockProps {
    /// Converts the inputs to the same type.
    fn convert_inputs(&self) -> (Option<Value>, Option<Value>) {
        let in1 = self.inputs().first().and_then(|input| input.get_value());
        let in2 = self.inputs().get(1).and_then(|input| input.get_value());

        let in2 = if let (Some(in1), Some(in2)) = (in1, in2) {
            match convert_value(in1, in2.clone()) {
                Ok(input2) => Some(input2),
                Err(_) => None,
            }
        } else {
            in2.cloned()
        };

        (in1.cloned(), in2)
    }
}
