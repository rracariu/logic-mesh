// Copyright (c) 2022-2024, Radu Racariu.

//! Time-related blocks.

mod calendar;
mod now;
mod schedule;
mod sun;

// Re-export implementations working with inputs and outputs
pub use calendar::Calendar;
pub use now::Now;
pub use schedule::Schedule;
pub use sun::Sun;
