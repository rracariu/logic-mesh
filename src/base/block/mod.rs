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
