//! The acts about a setup that was already on this machine.
//!
//! Apart from the table that names them for the reason the household's requests are:
//! what these read off the carrier is one field, and the group is going to grow. Three
//! of the four modes a survey describes — importing, standing beside, replacing — are
//! not offered yet, and each will arrive here as a name of its own rather than as an
//! argument to this one, because standing a second stack up and stopping somebody's are
//! not the same act under a flag.
//!
//! Adopting is the only one built. Unconfirmed it says what it would come to and writes
//! nothing, so what a browser agrees to is what it was shown; confirming is the operator
//! saying they have backed up the databases it named.

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
mod tests {
    use super::{about_a_setup_already_here, named, EVERY};
    use crate::actions::named::OFFERED;

    /// The names a browser may ask for and the modes the survey offers are one list.
    /// Two lists would drift the first time a fifth mode was added — and the drift would
    /// be a mode the survey offers that no browser can reach, which reads as a mode that
    /// does not work rather than one that was never wired up.
    #[test]
    fn every_mode_the_survey_offers_is_an_action_a_browser_can_ask_for() {
        for mode in EVERY {
            let name = format!("migrate-{}", mode.slug());
            assert!(OFFERED.contains(&name.as_str()), "{name} is offered");
        }
    }

    /// And nothing offered under that prefix is a mode nobody has heard of.
    #[test]
    fn every_migration_action_offered_names_a_mode() {
        for offered in OFFERED.iter().filter(|name| name.starts_with("migrate-")) {
            assert!(named(offered).is_some(), "{offered} names a mode");
        }
    }

    /// A name that is not one of them is not one of them.
    #[test]
    fn an_action_about_something_else_names_no_mode() {
        assert!(named("uninstall").is_none());
        assert!(!about_a_setup_already_here("uninstall"));
    }
}
