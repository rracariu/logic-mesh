// Copyright (c) 2022-2026, Radu Racariu.

//! Pin-level quality status attached to every value flowing on a watch
//! channel. Mirrors a subset of Project Haystack's `curStatus` tag and
//! OPC UA quality codes.
//!
//! For Phase 1 of fault propagation the status is essentially block-level
//! — every output of a faulted block emits `Fault`, and a consumer that
//! drains a `Fault`-status value faults itself. Phase 2 will introduce
//! per-pin granularity.

use serde::{Deserialize, Serialize};

/// Quality of a value at the moment it crosses a watch channel.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Phase 1 doesn't set this anywhere; reserved for staleness checks.
    Stale,
}

impl Status {
    /// Returns `true` if the status is `Ok`.
    pub fn is_ok(&self) -> bool {
        matches!(self, Status::Ok)
    }

    /// Returns `true` if the status is `Fault`.
    pub fn is_fault(&self) -> bool {
        matches!(self, Status::Fault)
    }
}
