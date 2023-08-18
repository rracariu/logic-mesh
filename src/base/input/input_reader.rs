// Copyright (c) 2022-2023, Radu Racariu.

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

    /// Reads the connected block inputs.
    /// This would only complete when at least one input has data.
    ///
    /// # Returns
    /// The index of the input that received a value.
    async fn read_inputs_until_ready(&mut self) -> Option<usize>;

    /// Waits for any input to have data.
    async fn wait_on_inputs(&mut self, timeout: Duration) -> Option<usize>;
}
