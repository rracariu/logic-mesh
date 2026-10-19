// Copyright (c) 2022-2024, Radu Racariu.

//! Miscellaneous blocks.

mod change_of_value;
mod derivative;
mod ema;
mod has_value;
mod integrator;
mod moving_average;
mod parse_bool;
mod parse_number;
mod random;
mod sample_hold;
mod sinewave;

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
