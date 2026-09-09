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

use super::Arguments;

/// Every act about a setup that was already here.
const ABOUT: [&str; 4] = [
    "migrate-adopt",
    "migrate-beside",
    "migrate-replace",
    "migrate-import",
];

/// Whether this action is about a setup that was already on the machine.
pub(super) fn about_a_setup_already_here(action: &str) -> bool {
    ABOUT.contains(&action)
}

/// What the action asks the core for.
///
/// Nothing here can be missing: the one field it reads is a confirmation, and not
/// having confirmed is an answer rather than an omission.
pub(super) fn asked_for(action: &str, given: &Arguments) -> Command {
    match action {
        "migrate-beside" => Command::Migrate(MigrateAction::Beside {
            confirmed: given.confirm,
        }),
        "migrate-replace" => Command::Migrate(MigrateAction::Replace {
            confirmed: given.confirm,
        }),
        "migrate-import" => Command::Migrate(MigrateAction::Import {
            confirmed: given.confirm,
        }),
        _ => Command::Migrate(MigrateAction::Adopt {
            confirmed: given.confirm,
        }),
    }
}
