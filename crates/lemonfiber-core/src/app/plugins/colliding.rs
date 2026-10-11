use crate::error::codes::plugin::{ANSWERED, SPELLED_ALIKE};
use crate::error::{Problem, Remedy, State};

/// Refuse a plugin one of whose services would answer on a label another installed
/// plugin's service already answers on, before anything is written.
///
/// # Errors
///
/// Where the label is taken, naming it and whose it is.
pub(crate) fn unanswered(
    would: &crate::plugin::Installed,
    installed: &[crate::plugin::Installed],
) -> Result<(), Box<Problem>> {
    crate::plugin::label_taken(would, installed).map_or(Ok(()), |(label, plugin)| {
        Err(Box::new(
            Problem::new(
                ANSWERED,
                format!(
                    "{} would answer on {label}, which {plugin} already does",
                    would.plugin
                ),
                "Nothing was written. The stack's proxy will not start with two sites at one \
                 address, and every household route would go down with it.",
                Remedy::new(format!(
                    "Give {}'s service another hostname in its manifest, or remove {plugin} first",
                    would.plugin
                )),
            )
            .in_state(State::Guided),
        ))
    })
}

/// Refuse a plugin one of whose services would be named as another installed plugin's
/// service already is, before anything is written.
///
/// # Errors
///
/// Where the name is taken, naming both services and whose the other is.
pub(crate) fn unshared(
    would: &crate::plugin::Installed,
    installed: &[crate::plugin::Installed],
) -> Result<(), Box<Problem>> {
    crate::plugin::spelled_alike(would, installed).map_or(Ok(()), |(ours, theirs, plugin)| {
        Err(Box::new(
            Problem::new(
                SPELLED_ALIKE,
                format!(
                    "{}'s service {ours} would be named as {plugin}'s {theirs} already is",
                    would.plugin
                ),
                "Nothing was written. lemonfiber writes a plugin's container under its \
                 service's name and keeps what the service holds in settings named after it, \
                 once case and \"-\" or \"_\" are set aside, so the two would be one container \
                 sharing one credential.",
                Remedy::new(format!(
                    "Remove {plugin} first, or install a version of {} whose service is named \
                     otherwise",
                    would.plugin
                )),
            )
            .in_state(State::Guided),
        ))
    })
}
