//! Verifies that native blocks can be defined in a downstream crate that
//! renames the `logic-mesh` dependency, using the
//! `#[logic_mesh(crate = "path")]` escape hatch.

use mesh_renamed::base::block::Block;
use mesh_renamed::base::input::{InputProps, input_reader::InputReader};
use mesh_renamed::base::output::Output;
use mesh_renamed::blocks::{InputImpl, OutputImpl};
use mesh_renamed::{BlockProps, block};

/// Negates the value of its numeric input.
#[block]
#[derive(BlockProps, Debug)]
#[logic_mesh(crate = "mesh_renamed")]
#[dis = "Negate"]
#[library = "downstream_renamed"]
#[category = "math"]
pub struct Negate {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Negate {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out.set((-num).into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_renamed::base::block::BlockProps;

    #[test]
    fn desc_reflects_attributes() {
        use mesh_renamed::base::block::BlockStaticDesc;

        let desc = <Negate as BlockStaticDesc>::desc();
        assert_eq!(desc.name, "Negate");
        assert_eq!(desc.library, "downstream_renamed");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);
    }

    #[tokio::test]
    async fn executes_and_negates_input() {
        let mut block = Negate::new();

        assert_eq!(block.inputs().len(), 1);

        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((21.into(), mesh_renamed::base::Status::Ok))
            .unwrap();

        block.execute().await;
        assert_eq!(block.out.value, (-21).into());
    }
}
