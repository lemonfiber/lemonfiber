//! What an act on an installed plugin is refused with where nothing by that name is.

use crate::error::{Code, Problem, Remedy, State};
use crate::plugin::{Installed, Register};

/// The plugin installed as `plugin`, or the refusal [`not_installed`] makes as `code`.
///
/// # Errors
///
/// Where nothing by that name is installed.
pub(super) fn named(
    held: &Register,
    plugin: &str,
    code: Code,
    nothing: &str,
) -> Result<Installed, Box<Problem>> {
    held.installed()
        .iter()
        .find(|one| one.plugin == plugin)
        .cloned()
        .ok_or_else(|| Box::new(not_installed(code, nothing, plugin, held.installed())))
}

/// Nothing by the name `plugin` is installed, refused as `code`, with `nothing` saying
/// what was not done.
///
/// Names what is, because the commonest reason to reach this is a name spelled the way
/// an operator remembers it rather than the way the plugin declares it — and a refusal
/// that says only *no* leaves them to go and look the list up themselves.
fn not_installed(code: Code, nothing: &str, plugin: &str, held: &[Installed]) -> Problem {
    let names: Vec<&str> = held.iter().map(|one| one.plugin.as_str()).collect();
    let meaning = if names.is_empty() {
        format!("{nothing} This machine has no plugins installed.")
    } else {
        format!("{nothing} What is installed: {}.", names.join(", "))
    };
    Problem::new(
        code,
        format!("{plugin} is not installed"),
        meaning,
        Remedy::new("Run lemonfiber plugin installed to see what is on this machine"),
    )
    .in_state(State::Guided)
}
