use crate::error::codes::plugin::{ANSWERED, EARLIER_TAKEN, SPELLED_ALIKE};
use crate::error::{Problem, Remedy, State};

/// Refuse a plugin that would answer on a label, be named as a service, or name an earlier
/// variable another installed plugin already does, before anything is written.
///
/// # Errors
///
/// The first of [`unanswered`], [`unshared`] and [`unclaimed`] to refuse it.
pub(crate) fn apart(
    would: &crate::plugin::Installed,
    installed: &[crate::plugin::Installed],
) -> Result<(), Box<Problem>> {
    unanswered(would, installed)?;
    unshared(would, installed)?;
    unclaimed(would, installed)
}

/// Refuse a plugin one of whose services would answer on a label another installed
/// plugin's service already answers on, before anything is written.
///
/// # Errors
///
/// Where the label is taken, naming it and whose it is.
fn unanswered(
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
fn unshared(
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

/// Refuse a plugin one of whose settings or secrets names, in `was`, a variable another
/// installed plugin's setting or secret names, before anything is written.
///
/// # Errors
///
/// Where the variable is taken, naming both and whose the other is.
fn unclaimed(
    would: &crate::plugin::Installed,
    installed: &[crate::plugin::Installed],
) -> Result<(), Box<Problem>> {
    crate::plugin::earlier::taken(would, installed).map_or(Ok(()), |(ours, theirs, plugin)| {
        Err(Box::new(
            Problem::new(
                EARLIER_TAKEN,
                format!(
                    "{}'s {} names {} as the variable it was kept under, which {plugin}'s {} \
                     already names",
                    would.plugin,
                    ours.named(),
                    ours.key,
                    theirs.named()
                ),
                "Nothing was written. The stack's value under that variable would be filed \
                 under both.",
                Remedy::new(format!(
                    "Remove {plugin} first, or install a version of {} whose was names the \
                     variable its own setting was kept under",
                    would.plugin
                )),
            )
            .in_state(State::Guided),
        ))
    })
}
