//! Reading the wall clock.
//!
//! Time is a port because the journal timestamps every change and checks time
//! out. A test that has to wait for a timeout to elapse is a slow test that
//! eventually becomes a flaky one.

use std::time::SystemTime;

/// Tells the time.
pub trait Clock: Send + Sync {
    /// The current wall-clock time.
    fn now(&self) -> SystemTime;
}

/// A monotonic clock a long copy measures its pace by and waits on.
pub trait Stopwatch: Send + Sync {
    /// The time since this stopwatch started.
    fn elapsed(&self) -> std::time::Duration;

    /// Wait for `span` to pass.
    fn wait(&self, span: std::time::Duration);
}
