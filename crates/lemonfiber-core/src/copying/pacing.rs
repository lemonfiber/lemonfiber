//! Holding a backup to the operator's limit.

use std::time::Duration;

pub use lemonfiber_ports::time::Stopwatch;

/// The machine's monotonic clock, waited on by sleeping the thread.
pub struct Monotonic(std::time::Instant);

impl Monotonic {
    /// A stopwatch started now.
    #[must_use]
    pub fn started() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Stopwatch for Monotonic {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn wait(&self, span: Duration) {
        std::thread::sleep(span);
    }
}

/// The bytes a copy has moved, against the most it may move a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Throttle {
    limit: u64,
    moved: u64,
}

impl Throttle {
    /// A throttle at `limit` bytes a second, nothing moved yet.
    pub(crate) const fn at(limit: u64) -> Self {
        Self { limit, moved: 0 }
    }

    /// The limit, in bytes a second.
    pub(crate) const fn limit(self) -> u64 {
        self.limit
    }

    /// How long to wait, `elapsed` into the copy and `bytes` more moved, before it may move more.
    pub(crate) fn after(&mut self, bytes: u64, elapsed: Duration) -> Duration {
        const NANOS: u128 = Duration::from_secs(1).as_nanos();
        self.moved = self.moved.saturating_add(bytes);
        let due = u128::from(self.moved) * NANOS / u128::from(self.limit.max(1));
        Duration::from_nanos(u64::try_from(due).unwrap_or(u64::MAX)).saturating_sub(elapsed)
    }
}
