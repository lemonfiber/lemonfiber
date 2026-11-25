//! The one record a rehearsal keeps of its own.

use crate::app::{Command, Queueing};

/// The record a rehearsal of `command` keeps of its own, by its file name beside the
/// configuration; nothing for every command but one.
///
/// A rehearsed `queue heal` keeps the run it would have taken, every act `would`, so the
/// queue's read shows it. It advances no strike, journals nothing, and tells no curator
/// or download client anything.
#[must_use]
pub const fn keeps_its_own_record(command: &Command) -> Option<&'static str> {
    match command {
        Command::Queue(Queueing::Heal) => Some(crate::config::paths::HEALING),
        _ => None,
    }
}
