// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block trait and associated types
//!

pub mod connect;
pub mod desc;
pub mod props;

pub use connect::BlockConnect;
pub use desc::BlockDesc;
pub use props::BlockDescAccess;
pub use props::BlockProps;

/// Determines the state a block is in
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum BlockState {
    #[default]
    Stopped,
    Running,
    Fault,
    Terminate,
}

pub trait Block: BlockConnect {
    async fn execute(&mut self);
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{Input, InputProps},
        output::Output,
    };

    use super::mock::{InputImpl, OutputImpl};

    use libhaystack::val::{kind::HaystackKind, Value};

    #[block]
    #[derive(BlockProps, Debug)]
    #[name = "Test"]
    #[library = "test"]
    #[category = "test"]
    #[input(kind = "Number", count = 16)]
    struct Test {
        #[input(kind = "Number")]
        user_defined: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }

    impl Block for Test {
        async fn execute(&mut self) {
            self.out.value = Value::make_int(42);
        }
    }

    #[test]
    fn test_block_props_declared_inputs() {
        let test_block = &Test::new() as &dyn BlockProps<Reader = String, Writer = String>;

        assert_eq!(test_block.desc().name, "Test");
        assert_eq!(test_block.desc().library, "test");
        assert_eq!(test_block.state(), BlockState::Stopped);
        assert_eq!(test_block.inputs().len(), 17);
        assert_eq!(test_block.outputs().len(), 1);

        assert_eq!(
            test_block
                .inputs()
                .iter()
                .filter(|input| input.name().starts_with("in"))
                .count(),
            16
        );

        assert!(test_block
            .inputs()
            .iter()
            .filter(|input| input.name().starts_with("in"))
            .enumerate()
            .all(|(i, input)| input.name() == format!("in{}", i)));

        assert!(test_block
            .inputs()
            .iter()
            .all(|i| i.kind() == &HaystackKind::Number));

        assert!(test_block.outputs()[0].desc().name == "out");
        assert!(test_block.outputs()[0].desc().kind == HaystackKind::Number);
        assert!(!test_block.outputs()[0].is_connected());
    }

    #[test]
    fn test_block_outputs() {
        let test_block = &Test::new() as &dyn BlockProps<Reader = String, Writer = String>;

        assert_eq!(test_block.outputs().len(), 1);
        assert_eq!(test_block.outputs()[0].desc().name, "out");
        assert_eq!(test_block.outputs()[0].desc().kind, HaystackKind::Number);
        assert!(!test_block.outputs()[0].is_connected());
    }
}
