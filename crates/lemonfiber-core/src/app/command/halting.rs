//! Stopping named services and leaving the rest running.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use super::super::Waiting;
use super::{Command, Teardown};

/// Stopping named services, and whether to wait for their downloads first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halting {
    /// The forms the services belong to; none means the whole stack.
    pub forms: Vec<String>,
    /// The services to stop.
    pub services: Vec<String>,
    /// Whether anything still downloading inside these services is let finish first.
    pub wait: Waiting,
}

impl Command {
    /// A stop of `forms`: tearing them down where `services` names none, and stopping the
    /// named services alone where it names some, waiting for downloads first as `wait` says.
    #[must_use]
    pub fn stopping(forms: Vec<String>, services: Vec<String>, wait: Waiting) -> Self {
        if services.is_empty() {
            Self::Down(Teardown { forms, wait })
        } else {
            Self::Halt(Halting {
                forms,
                services,
                wait,
            })
        }
    }
}
