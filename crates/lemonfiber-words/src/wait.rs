//! How long a wait reads.

use std::time::Duration;

/// A duration in the coarsest unit that still says something useful — nobody waiting on a
/// download wants it to the second.
#[must_use]
pub fn spell_out(left: Duration) -> String {
    let seconds = left.as_secs();
    if seconds >= 3600 {
        return format!("{}h", seconds / 3600);
    }
    if seconds >= 60 {
        return format!("{}m", seconds / 60);
    }
    format!("{seconds}s")
}

#[cfg(test)]
mod tests;
