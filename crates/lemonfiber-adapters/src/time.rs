//! Reading this machine's clock.

use std::time::SystemTime;

use lemonfiber_ports::time::Clock;

/// Reads the operating system's wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct System;

impl Clock for System {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
mod tests;
