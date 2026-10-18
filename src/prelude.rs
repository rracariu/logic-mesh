// Copyright (c) 2022-2026, Radu Racariu.

//! The traits, macros and pin types needed to define, wire and run
//! blocks, for a glob import.
//!
//! ```
//! use logic_mesh::prelude::*;
//!
//! /// Doubles its input.
//! #[block]
//! #[derive(BlockProps, Debug)]
//! #[category = "math"]
//! struct Double {
//!     #[input(name = "in", kind = "Number")]
//!     input: InputImpl,
//!     #[output(kind = "Number")]
//!     out: OutputImpl,
//! }
//!
//! impl Block for Double {
//!     async fn execute(&mut self) {
//!         self.read_inputs_until_ready().await;
//!         if let Some(Ok(num)) = self.input.get_value().map(f64::try_from) {
//!             self.out.set((num * 2.0).into());
//!         }
//!     }
//! }
//!
//! let double = Double::new();
//! assert_eq!(double.inputs()[0].name(), "in");
//! ```
//!
//! Everything here also has its own path, which the documentation of
//! each item shows; the prelude only gathers them.

pub use crate::base::block::{Block, BlockConnect, BlockProps, BlockStaticDesc};
pub use crate::base::engine::Engine;
pub use crate::base::input::{InputProps, InputReader};
pub use crate::base::output::{Output, OutputProps};
pub use crate::blocks::{InputImpl, OutputImpl};
pub use crate::{BlockProps, block};
