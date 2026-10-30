// Copyright (c) 2022-2026, Radu Racariu.

//! The scan interval: the period at which blocks that poll check their
//! state, configured per engine.
//!
//! An engine holds a [`ScanInterval`] and runs every block actor task
//! inside [`ScanInterval::scope`], which makes it the task-local value
//! [`scan_interval_millis`] reads. Every task of one engine shares the
//! same cell, so a change reaches the blocks already scheduled, and the
//! tasks of another engine never see it.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// The scan interval an engine starts with.
pub(crate) const DEFAULT_SCAN_INTERVAL: Duration = Duration::from_millis(200);

tokio::task_local! {
    /// The scan interval of the engine whose task is running.
    static SCAN_INTERVAL: ScanInterval;
}

/// An engine's scan interval, in whole milliseconds, shared with the
/// tasks it runs.
#[derive(Debug, Clone)]
pub(crate) struct ScanInterval(Arc<AtomicU64>);

impl Default for ScanInterval {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(millis(DEFAULT_SCAN_INTERVAL))))
    }
}

impl ScanInterval {
    /// The current interval.
    pub(crate) fn get(&self) -> Duration {
        Duration::from_millis(self.0.load(Ordering::Relaxed))
    }

    /// Replaces the interval, for every task running in this scope.
    pub(crate) fn set(&self, interval: Duration) {
        self.0.store(millis(interval), Ordering::Relaxed);
    }

    /// Runs `task` with this as the interval [`scan_interval_millis`]
    /// reports.
    pub(crate) fn scope<F: Future>(&self, task: F) -> impl Future<Output = F::Output> + use<F> {
        SCAN_INTERVAL.scope(self.clone(), task)
    }
}

/// The scan interval, in milliseconds, of the engine running the
/// current task — or [`DEFAULT_SCAN_INTERVAL`] outside an engine, as
/// when a block is evaluated directly.
pub(crate) fn scan_interval_millis() -> u64 {
    SCAN_INTERVAL
        .try_with(|interval| interval.0.load(Ordering::Relaxed))
        .unwrap_or(millis(DEFAULT_SCAN_INTERVAL))
}

/// `interval` in whole milliseconds, saturating.
fn millis(interval: Duration) -> u64 {
    u64::try_from(interval.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn a_scope_reports_its_interval_and_later_changes() {
        let interval = ScanInterval::default();
        interval.set(Duration::from_millis(50));

        let observed = interval
            .scope(async {
                let first = scan_interval_millis();
                tokio::task::yield_now().await;
                first
            })
            .await;
        assert_eq!(observed, 50);

        interval.set(Duration::from_millis(75));
        assert_eq!(interval.scope(async { scan_interval_millis() }).await, 75);
        assert_eq!(interval.get(), Duration::from_millis(75));
    }

    #[test]
    fn outside_a_scope_the_default_applies() {
        assert_eq!(
            scan_interval_millis(),
            DEFAULT_SCAN_INTERVAL.as_millis() as u64
        );
    }
}
