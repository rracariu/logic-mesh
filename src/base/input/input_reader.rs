// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::time::Duration;

use crate::base::block::Block;

/// Specifies the protocol for reading
/// block inputs
pub trait InputReader: Block {
    /// Reads the connected block inputs.
    ///
    /// # Returns
    /// The index of the input that received a value.
    async fn read_inputs(&mut self) -> Option<usize>;

    /// Waits for any input to have data.
    async fn wait_on_inputs(&mut self, timeout: Duration);
}
