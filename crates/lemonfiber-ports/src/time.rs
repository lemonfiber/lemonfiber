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
