//! Verifies that native blocks can be defined in a downstream crate
//! using the same `#[block]` / `#[derive(BlockProps)]` machinery as the
//! built-in blocks.

use logic_mesh::base::block::Block;
use logic_mesh::base::input::{InputProps, input_reader::InputReader};
use logic_mesh::base::output::Output;
use logic_mesh::blocks::{InputImpl, OutputImpl};
use logic_mesh::{BlockProps, block};

/// Doubles the value of its numeric input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Double"]
#[library = "downstream"]
#[category = "math"]
pub struct Double {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Double {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out.set((num * 2.0).into());
        }
    }
}

/// Same as [`Double`] but declares its pins with fully qualified paths.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Triple"]
#[library = "downstream"]
#[category = "math"]
pub struct Triple {
    #[input(name = "in", kind = "Number")]
    pub input: logic_mesh::blocks::InputImpl,
    #[output(kind = "Number")]
    pub out: logic_mesh::blocks::OutputImpl,
}

impl Block for Triple {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out.set((num * 3.0).into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use logic_mesh::base::block::BlockProps;

    #[test]
    fn desc_reflects_attributes() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Double as BlockStaticDesc>::desc();
        assert_eq!(desc.name, "Double");
        assert_eq!(desc.dis, "Double");
        assert_eq!(desc.library, "downstream");
        assert_eq!(desc.category, "math");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].name, "in");
        assert_eq!(desc.outputs.len(), 1);
        assert_eq!(desc.outputs[0].name, "out");
    }

    #[tokio::test]
    async fn executes_and_doubles_input() {
        let mut block = Double::new();

        assert_eq!(block.inputs().len(), 1);
        assert_eq!(block.outputs().len(), 1);

        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((21.into(), logic_mesh::base::Status::Ok))
            .unwrap();

        block.execute().await;
        let expected: logic_mesh::Value = 42.into();
        assert_eq!(block.out.value, expected);
    }

    #[tokio::test]
    async fn fully_qualified_pin_types_work() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Triple as BlockStaticDesc>::desc();
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);

        let mut block = Triple::new();
        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((7.into(), logic_mesh::base::Status::Ok))
            .unwrap();

        block.execute().await;
        assert_eq!(block.out.value, 21.into());
    }

    #[test]
    fn registers_in_runtime_registry() {
        logic_mesh::blocks::registry::register::<Double>();

        let registered = logic_mesh::blocks::registry::list_registered_blocks();
        assert!(
            registered
                .iter()
                .any(|desc| desc.name == "Double" && desc.library == "downstream")
        );
    }

    #[tokio::test]
    async fn registered_block_evals_by_name() {
        logic_mesh::blocks::registry::register::<Double>();

        let result = logic_mesh::blocks::registry::eval_static_block(
            "Double",
            Some("downstream"),
            vec![21.into()],
        )
        .await;
        assert_eq!(result.unwrap(), vec![logic_mesh::Value::from(42)]);
    }

    #[test]
    fn registered_block_schedules_on_engine() {
        logic_mesh::blocks::registry::register::<Double>();

        let mut eng = logic_mesh::single_threaded::SingleThreadedEngine::new();
        let id =
            logic_mesh::blocks::registry::schedule_block("Double", Some("downstream"), &mut eng)
                .expect("schedule registered block");

        assert!(
            eng.block_handles()
                .iter()
                .any(|b| *b.id() == id && b.desc().name == "Double")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registered_block_schedules_on_multi_threaded_engine() {
        logic_mesh::blocks::registry::register::<Double>();

        let mut eng = logic_mesh::multi_threaded::MultiThreadedEngine::new();
        logic_mesh::blocks::registry::schedule_block_send("Double", Some("downstream"), &mut eng)
            .expect("schedule registered block on MT engine");
    }

    #[test]
    fn generic_schedule_on_multi_threaded_engine_errors() {
        logic_mesh::blocks::registry::register::<Double>();

        let mut eng = logic_mesh::multi_threaded::MultiThreadedEngine::new();
        let err =
            logic_mesh::blocks::registry::schedule_block("Double", Some("downstream"), &mut eng)
                .expect_err("trait-path scheduling on the MT engine should error, not panic");
        assert!(err.to_string().contains("schedule_send"));
    }
}
