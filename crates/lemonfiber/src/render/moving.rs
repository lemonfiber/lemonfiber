//! A move in place on offer, named wherever a run would start or update the stack.

use lemonfiber_core::app::plugins::MoveOffered;
use lemonfiber_core::app::InstallOffered;
use lemonfiber_core::model::LifecycleReport;
use lemonfiber_core::update::run::Moved;

use super::Lines;

/// Each part this release runs as a plugin that the stack still runs itself, with the
/// reading that takes the move, or nothing where none is on offer.
pub(crate) fn offered(moves: &[MoveOffered]) -> Lines {
    let mut lines = Lines::default();
    for one in moves {
        lines.spaced(format!(
            "On offer: this release runs {} as a plugin, and it still runs as the stack's own \
             service. Moving it keeps its container, configuration, data and credentials; \
             `lemonfiber update stack` shows the move, and confirming the update takes it.",
            one.plugin
        ));
        if let Some(adapter) = &one.configured_by {
            lines.put(format!(
                "Until it moves, {} runs without {adapter}, which writes the configuration it \
                 reads, so it starts without what that holds, the credentials it reaches other \
                 services with included.",
                one.service
            ));
        }
    }
    lines
}

/// Each part this build carries that a start would install, offline, because its forms
/// hold it and this machine has never run it, and the offer that takes them all where it
/// is named, with the command `answering` it.
pub(crate) fn installing(installs: &[InstallOffered], answering: Option<(&str, &str)>) -> Lines {
    let mut lines = Lines::default();
    for one in installs {
        lines.spaced(match &one.why {
            None => format!(
                "On offer: this release runs {} as a plugin, and this machine has never run \
                 it. Starting installs it from this build, offline, before anything else of \
                 the form is written or started.",
                one.plugin
            ),
            Some(why) => format!(
                "On offer: this release runs {} as a plugin, and its install could not be \
                 read: {why}",
                one.plugin
            ),
        });
    }
    if let Some((action, offer)) = answering.filter(|_| !installs.is_empty()) {
        lines.put(format!(
            "Run the same `lemonfiber {action}` with `--offer {offer}` to install and start."
        ));
    }
    lines
}

/// What a start that answered none of the installs it offers says: that it wrote and
/// started nothing, and what is on offer, which is the whole of what it has to say.
/// Nothing for any other report.
pub(crate) fn unanswered(report: &LifecycleReport) -> Option<Lines> {
    if report.rehearsed || !report.command.is_empty() || report.installing.is_empty() {
        return None;
    }
    let mut lines = Lines::default();
    lines.put(format!(
        "{}: nothing was written or started — an install is on offer",
        report.action
    ));
    lines.extend(installing(&report.installing, answering(report)));
    Some(lines)
}

/// What a rehearsed start offers to install, and the offer that takes it.
pub(crate) fn rehearsed(report: &LifecycleReport) -> Lines {
    if report.rehearsed {
        installing(&report.installing, answering(report))
    } else {
        Lines::default()
    }
}

/// The command a report's offer answers, and the offer.
fn answering(report: &LifecycleReport) -> Option<(&str, &str)> {
    report
        .offer
        .as_deref()
        .map(|offer| (report.action.as_str(), offer))
}

/// Each move in place a confirmed update took, or could not, with why.
pub(crate) fn moved(moved: &[Moved]) -> Lines {
    let mut lines = Lines::default();
    for one in moved {
        lines.spaced(match &one.why {
            None if one.taken => format!(
                "Moved: {} now runs as its plugin in the place of the stack's own {}.",
                one.plugin, one.service
            ),
            why => format!(
                "Not moved: {} still runs as the stack's own service{}.",
                one.service,
                why.as_ref()
                    .map(|why| format!(", because {why}"))
                    .unwrap_or_default()
            ),
        });
    }
    lines
}

/// Each of lemonfiber's own guards a run moved onto its plugin for good, has still to
/// move, or could not move, with why.
pub(crate) fn guards(guards: &[Moved]) -> Lines {
    let mut lines = Lines::default();
    for one in guards {
        lines.spaced(match &one.why {
            None if one.taken => format!(
                "Moved for good: {} now runs as its plugin in the place of the stack's own {}. \
                 Neither an undo nor a removal puts it back.",
                one.plugin, one.service
            ),
            None => format!(
                "To move: {} moves onto its plugin {} with the next run that leaves the stack \
                 running, for good and with no yes of its own.",
                one.service, one.plugin
            ),
            Some(why) => format!(
                "Not moved: {} still runs as the stack's own service, because {why}. It stays \
                 on offer for the next run that leaves the stack running.",
                one.service
            ),
        });
    }
    lines
}

#[cfg(test)]
mod tests;
