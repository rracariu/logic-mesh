// Copyright (c) 2022-2023, IntriSemantics Corp.

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
}

pub trait Block: BlockConnect {
    async fn execute(&mut self);
}

#[cfg(test)]
mod test {
    use std::pin::Pin;

    use uuid::Uuid;

    use crate::base::{
        block::{Block, BlockDesc, BlockProps, BlockState},
        input::{BaseInput, Input, InputProps, InputReceiver},
        link::{BaseLink, Link},
        output::{BaseOutput, Output},
    };

    use libhaystack::val::{kind::HaystackKind, Value};

    pub type InputImpl = BaseInput<String, String, BaseLink<String>>;

    impl InputImpl {
        pub fn new(name: &str, kind: HaystackKind, block_id: Uuid) -> Self {
            Self {
                name: name.to_string(),
                kind,
                block_id,
                ..Default::default()
            }
        }
    }

    impl Input for InputImpl {
        fn receiver(&mut self) -> Pin<Box<dyn InputReceiver + '_>> {
            Box::pin(async { None })
        }
    }

    pub type OutputImpl = BaseOutput<BaseLink<String>>;

    impl Default for OutputImpl {
        fn default() -> Self {
            Self::new(HaystackKind::Null, Uuid::default())
        }
    }

    impl Output for OutputImpl {
        type Tx = <InputImpl as InputProps>::Tx;
        fn add_link(&mut self, _link: BaseLink<Self::Tx>) {}

        fn remove_link(&mut self, _link: &dyn Link) {}

        fn set(&mut self, _value: Value) {}
    }

    #[test]
    fn test_block_props_declared_inputs() {
        #[block]
        #[derive(BlockProps, Debug, Default)]
        #[name = "Test"]
        #[library = "test"]
        #[input(kind = "Number", count = 16)]
        struct Test {
            #[output(kind = "Number")]
            out: OutputImpl,
        }

        impl Block for Test {
            async fn execute(&mut self) {
                self.out.value = Value::make_int(42);
            }
        }

        let test_block = &Test::new("Test") as &dyn BlockProps<Rx = String, Tx = String>;

        assert_eq!(test_block.desc().name, "Test");
        assert_eq!(test_block.desc().library, "test");
        assert_eq!(test_block.state(), BlockState::Stopped);
        assert_eq!(test_block.inputs().len(), 16);
        assert_eq!(test_block.outputs().len(), 1);

        assert!(test_block
            .inputs()
            .iter()
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
}
