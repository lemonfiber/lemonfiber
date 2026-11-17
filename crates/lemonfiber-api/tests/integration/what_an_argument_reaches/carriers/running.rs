//! Where a run a request names is carried in the command it reaches: the run an undo
//! puts back, and a run-and-exit service run at once.

use super::super::acting::STAMP;
use lemonfiber_api::actions::{Arguments, Timing};
use lemonfiber_core::app::Command;

pub(super) fn give_at(given: &mut Arguments) {
    given.at = Some(STAMP);
}

/// Whether the command names the run it was given.
pub(super) fn carries_at(command: &Command) -> bool {
    matches!(command, Command::Undo { run: Some(at) } if *at == STAMP)
}

pub(super) fn give_now(given: &mut Arguments) {
    given.now = Timing::AtOnce;
}

/// Whether the command runs a run-and-exit service at once.
pub(super) fn carries_now(command: &Command) -> bool {
    matches!(command, Command::Run { now: true, .. })
}
