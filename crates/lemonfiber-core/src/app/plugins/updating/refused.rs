//! What an update is refused with before it touches anything.

use crate::error::codes::plugin::{ANOTHER_PLUGIN, NOTHING_TO_UPDATE, STUCK};
use crate::error::{Problem, Remedy, State};
use crate::plugin::Installed;

/// No version of this plugin is installed, so there is nothing to replace.
pub(super) fn not_installed(plugin: &str) -> Problem {
    Problem::new(
        NOTHING_TO_UPDATE,
        format!("{plugin} is not installed, so there is nothing to update"),
        "Nothing was changed. An update replaces a version this machine already has.",
        Remedy::new("Install it instead: lemonfiber plugin install on the same source"),
    )
    .in_state(State::Guided)
}

/// The source holds a different plugin from the one the update named. The name asked
/// for is the asker's own words, so it is said through the sanitiser.
pub(super) fn another_plugin(named: &str, holds: &str) -> Problem {
    let named = crate::text::plain(named);
    Problem::new(
        ANOTHER_PLUGIN,
        format!("That source holds {holds}, not {named}"),
        format!(
            "Nothing was changed. An update puts a new version of {named} in place of the one \
             installed, and this source holds a different plugin."
        ),
        Remedy::new(format!(
            "Name a source that holds {named}, or install {holds} on its own"
        )),
    )
    .in_state(State::Guided)
}

/// The installed version's containers would not come off, and nothing else was touched.
pub(super) fn stuck(was: &Installed) -> Problem {
    Problem::new(
        STUCK,
        format!(
            "{} {} would not stop, so it was not updated",
            was.plugin, was.version
        ),
        format!(
            "Nothing else was changed: {} is still installed, its files are where they were, \
             and the record still names {}. The container engine was asked to take its \
             containers off and would not.",
            was.plugin, was.version
        ),
        Remedy::new("Check the container engine is running, then try the update again"),
    )
    .in_state(State::Guided)
}
