// Copyright (c) 2022-2024, Radu Racariu.

//!
//! Block implementing time-related functionality
//!

pub mod calendar;
pub mod now;
pub mod schedule;

// Re-export implementations working with inputs and outputs
pub use calendar::Calendar;
pub use now::Now;
pub use schedule::Schedule;
