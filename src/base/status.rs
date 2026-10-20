// Copyright (c) 2022-2026, Radu Racariu.

//! Pin-level quality status attached to every value flowing on a watch
//! channel. Mirrors a subset of Project Haystack's `curStatus` tag and
//! OPC UA quality codes.
//!
//! The status is set per block today: every output of a faulted block
//! emits [`Status::Fault`], and a consumer that drains a
//! [`Status::Fault`]-status value faults itself. Per-pin statuses may
//! follow; the enum is `#[non_exhaustive]` so they can be added.

use serde::{Deserialize, Serialize};

/// Quality of a value at the moment it crosses a watch channel.
///
/// `#[non_exhaustive]`: [`Status::Stale`] is reserved, and further
/// qualities may follow, so a `match` on it needs a wildcard arm.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Status {
    /// The producer asserts the value is current and trustworthy.
    #[default]
    Ok,
    /// The producer is in a fault state. The accompanying value is the
    /// last computed value, kept for context; consumers should not act
    /// on it.
    Fault,
    /// The producer has not delivered an update within the expected
    /// freshness window. The accompanying value is the last known good.
    /// Nothing sets it yet; reserved for staleness checks.
    Stale,
}

impl Status {
    /// Returns `true` if the status is [`Status::Ok`].
    pub fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }

    /// Returns `true` if the status is [`Status::Fault`].
    pub fn is_fault(&self) -> bool {
        matches!(self, Status::Fault)
    }
}
