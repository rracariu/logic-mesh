// Copyright (c) 2022-2024, Radu Racariu.

//!
//! Miscellaneous Blocks
//!

pub mod change_of_value;
pub mod derivative;
pub mod ema;
pub mod has_value;
pub mod integrator;
pub mod moving_average;
pub mod parse_bool;
pub mod parse_number;
pub mod random;
pub mod sample_hold;
pub mod sinewave;

pub use change_of_value::ChangeOfValue;
pub use derivative::Derivative;
pub use ema::Ema;
pub use has_value::HasValue;
pub use integrator::Integrator;
pub use moving_average::MovingAverage;
pub use parse_bool::ParseBool;
pub use parse_number::ParseNumber;
pub use random::Random;
pub use sample_hold::SampleHold;
pub use sinewave::SineWave;
