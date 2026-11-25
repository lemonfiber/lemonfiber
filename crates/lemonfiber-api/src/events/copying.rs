//! When the operator's stream says the backup or restore running on this machine.

use std::time::Duration;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::copying::{Copying, Running};
use lemonfiber_core::error::Problem;
use tokio::time::Instant;

use super::theirs::due;

/// How often a running copy is said while nothing about it changes but how far it has got.
pub const COPYING_EVERY: Duration = Duration::from_secs(5);

/// What the stream last said of the running copy, and when.
#[derive(Debug, Default)]
pub struct Heard {
    said: Option<Instant>,
    was: Option<Copying>,
}

impl Heard {
    /// What a read of the copy running now answered, where it is owed.
    pub fn heard(
        &mut self,
        answered: Result<Outcome, Box<Problem>>,
        joined: bool,
        at: Instant,
    ) -> Option<Copying> {
        let Ok(Outcome::Copying(now)) = answered else {
            return None;
        };
        self.due(now, joined, at)
    }

    /// The reading to say at `at`, where a listener `joined`, the copy started, ended or
    /// changed its state or part, or it runs and was last said [`COPYING_EVERY`] ago.
    fn due(&mut self, now: Copying, joined: bool, at: Instant) -> Option<Copying> {
        let changed = self.was.as_ref().is_none_or(|was| moved(was, &now));
        let every = now.running.is_some() && due(self.said, COPYING_EVERY, at);
        if !(joined || changed || every) {
            return None;
        }
        self.said = Some(at);
        self.was = Some(now.clone());
        Some(now)
    }
}

/// Whether a copy started, ended, or changed its state or the part it writes.
fn moved(was: &Copying, now: &Copying) -> bool {
    let mark = |running: &Running| {
        (
            running.what,
            running.started,
            running.part.clone(),
            running.state,
        )
    };
    was.running.as_ref().map(mark) != now.running.as_ref().map(mark)
}

#[cfg(test)]
mod tests;
