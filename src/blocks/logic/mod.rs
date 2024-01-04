// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Boolean logic blocks
//!

pub mod and;
mod binary;
pub mod eq;
pub mod gt;
pub mod gte;
pub mod latch;
pub mod lt;
pub mod lte;
pub mod neq;
pub mod not;
pub mod or;
pub mod xor;

pub use and::And;
pub use eq::Equal;
pub use gt::GreaterThan;
pub use gte::GreaterThanEq;
pub use latch::Latch;
pub use lt::LessThan;
pub use lte::LessThanEq;
pub use neq::NotEqual;
pub use not::Not;
pub use or::Or;
pub use xor::Xor;
