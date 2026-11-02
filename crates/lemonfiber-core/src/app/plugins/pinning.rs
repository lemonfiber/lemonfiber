//! Which source a plugin carrying the id of a part this build pins may come from.

use crate::error::codes::plugin::ANOTHER_SOURCE;
use crate::error::{Problem, Remedy, State};
use crate::plugin::first_party::{holds, FirstParty};
use crate::plugin::Installed;

/// Refuse a plugin whose id one of `trusted` pins where its manifest or its tree is not
/// that pin's, before anything is written.
///
/// # Errors
///
/// Where it is not, naming the id and saying this build carries it.
pub(crate) fn from_its_pin(would: &Installed, trusted: &[FirstParty]) -> Result<(), Box<Problem>> {
    if holds(trusted, would) || !trusted.iter().any(|one| one.plugin == would.plugin) {
        return Ok(());
    }
    Err(Box::new(
        Problem::new(
            ANOTHER_SOURCE,
            format!(
                "{} is the id of a plugin this build carries, and what was offered is not the \
                 tree its pin names",
                would.plugin
            ),
            format!(
                "Nothing was written. {} is installed and updated only from the tree this build \
                 carries or the release its pin names.",
                would.plugin
            ),
            Remedy::new(format!(
                "Install it with lemonfiber plugin install {}, or give a plugin of your own an id \
                 of its own",
                would.plugin
            )),
        )
        .in_state(State::Guided),
    ))
}

/// Refuse what `trusted` does not let `would` be: a pinned id from anywhere but its pin,
/// and a stranger moving one of the stack's own services in place.
///
/// # Errors
///
/// The first of the two it is refused for.
pub(crate) fn as_trusted(would: &Installed, trusted: &[FirstParty]) -> Result<(), Box<Problem>> {
    from_its_pin(would, trusted)?;
    super::replacing::unreplaced(would, trusted)
}

#[cfg(test)]
mod tests;
