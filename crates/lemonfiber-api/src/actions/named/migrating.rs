//! The acts about a setup that was already on this machine.
//!
//! Apart from the table that names them for the reason the household's requests are:
//! what these read off the carrier is one field. Each of the four modes a survey
//! describes — adopting, importing, standing beside, replacing — is a name of its own
//! rather than an argument to one, because standing a second stack up and stopping
//! somebody's are not the same act under a flag.
//!
//! Unconfirmed, each says what it would come to and changes nothing, so what a browser
//! agrees to is what it was shown; `confirm` is the agreement, carried into the command
//! as the core's `confirmed`.

use lemonfiber_core::app::{Command, MigrateAction};
use lemonfiber_core::migration::mode::{Mode, EVERY};

use super::Arguments;

/// Whether this action is about a setup that was already on the machine.
pub(super) fn about_a_setup_already_here(action: &str) -> bool {
    named(action).is_some()
}

/// The mode this action names, where it names one.
fn named(action: &str) -> Option<Mode> {
    EVERY
        .into_iter()
        .find(|mode| action == format!("migrate-{}", mode.slug()))
}

/// What the action asks the core for.
///
/// Nothing here can be missing: the one field it reads is a confirmation, and not
/// having confirmed is an answer rather than an omission.
pub(super) fn asked_for(action: &str, given: &Arguments) -> Command {
    let mode = named(action).unwrap_or_default();
    Command::Migrate(MigrateAction::Act {
        mode,
        confirmed: given.confirm,
    })
}

#[cfg(test)]
mod tests;
