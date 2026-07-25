//! Reading this machine's clock.

use std::time::SystemTime;

use crate::ports::time::Clock;

/// Reads the operating system's wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct System;

impl Clock for System {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
mod tests {
    use super::{Clock, System};

    #[test]
    fn reports_a_time_that_moves_forwards() {
        let before = System.now();
        let after = System.now();
        assert!(after >= before);
    }
}
