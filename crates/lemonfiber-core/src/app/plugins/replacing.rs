//! Which plugins may move one of the stack's own services in place.

use crate::error::codes::plugin::STRANGER_REPLACING;
use crate::error::{Problem, Remedy, State};

/// Refuse a plugin that is not one of `trusted` and declares a service replacing one the
/// stack ships, before anything is written.
///
/// # Errors
///
/// Where it declares one, naming the service and the stack service it would replace.
pub(crate) fn unreplaced(
    would: &crate::plugin::Installed,
    trusted: &[crate::plugin::first_party::FirstParty],
) -> Result<(), Box<Problem>> {
    if crate::plugin::first_party::holds(trusted, would) {
        return Ok(());
    }
    would
        .services
        .iter()
        .find_map(|placed| {
            placed
                .replaces
                .as_deref()
                .map(|held| (&placed.service, held))
        })
        .map_or(Ok(()), |(service, held)| {
            Err(Box::new(
                Problem::new(
                    STRANGER_REPLACING,
                    format!(
                        "{}'s service {service} would replace the stack's own {held}, and {} is \
                         not a plugin this build embeds as first-party",
                        would.plugin, would.plugin
                    ),
                    "Nothing was written. Only a first-party plugin moves one of the stack's own \
                     services in place.",
                    Remedy::new(format!(
                        "Install {} without {service}'s replaces, or use the first-party plugin \
                         this build names for {held}",
                        would.plugin
                    )),
                )
                .in_state(State::Guided),
            ))
        })
}
