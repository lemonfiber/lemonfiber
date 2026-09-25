//! What is already on this machine, on a terminal.
//!
//! The survey reads in the order somebody decides in: what is here, then what is in
//! the way, then what taking it over would cost, then what may be done about it. The
//! sentence that nothing was changed comes last, because it is what they are left
//! holding.
//!
//! Each section is its own function. They answer separate questions and an operator
//! reads whichever of them their own machine put in front of them, so a survey of a
//! bare machine and one of a full stack are the same code taking different turns
//! rather than one function knowing about every case at once.

use lemonfiber_core::model::{
    AdoptReport, BesideReport, ImportReport, MigrationReport, RecordReport, ReplaceReport,
    UnsupportedReport,
};

use lemonfiber_core::reconfigure::Stance;

use super::Lines;

/// What is already on this machine, before anything is proposed.
///
/// Six sections, each its own function. They are separate because they answer separate
/// questions — what is here, what collides, what taking it over costs, what is left
/// alone, what may be done, and where a second copy would listen — and an operator
/// reads whichever of them their situation put in front of them.
pub(super) fn migration(report: &MigrationReport) -> Lines {
    let mut lines = Lines::default();
    if !report.read {
        // Could not look, which is not the same as found nothing, and the difference
        // decides whether it is safe to stand anything up here.
        lines.put("could not read what is on this machine, so nothing is ruled out".to_owned());
        return lines;
    }
    standing_here(report, &mut lines);
    clashes(report, &mut lines);
    layout(report, &mut lines);
    taking_over(report, &mut lines);
    choices(report, &mut lines);
    listed(
        &report.unsupported,
        "found, and left exactly as it is:",
        &mut lines,
    );
    listed(
        &report.not_carried,
        "no migration carries these across:",
        &mut lines,
    );
    lines.put(String::new());
    lines.put("nothing was changed".to_owned());
    lines
}

/// What adopting a setup already here came to, or would come to.
///
/// What is said instead of everything else, where an act was refused.
///
/// A refusal is the whole answer: an operator told they cannot do this needs the reason,
/// and nothing else on the screen is useful to them. One place rather than one per act,
/// so the four cannot come to disagree about that.
fn turned_away(refusal: Option<&String>, what: &str) -> Option<Lines> {
    let refused = refusal?;
    let mut lines = Lines::default();
    lines.put(format!("{what}: {refused}"));
    Some(lines)
}

pub(super) fn adoption(report: &AdoptReport) -> Lines {
    if let Some(said) = turned_away(report.refusal.as_ref(), "not adopting") {
        return said;
    }
    let mut lines = Lines::default();
    let named = report.project.clone().unwrap_or_default();
    if report.stance == Stance::Applied {
        lines.put(format!("lemonfiber now manages {named}"));
    } else {
        lines.put(format!("adopting {named} would:"));
    }

    for service in &report.upgrades {
        lines.put(format!(
            "  upgrade {} from {} to {}, which nothing walks back",
            service.service, service.existing, service.ours
        ));
    }
    if !report.back_up.is_empty() && report.stance != Stance::Applied {
        lines.put(String::new());
        lines.put("back up these before confirming:".to_owned());
        for path in &report.back_up {
            lines.put(format!("  {path}"));
        }
    }
    lines.put(String::new());
    if report.stance == Stance::Applied {
        lines.put("nothing was started, stopped, or moved".to_owned());
    } else {
        lines.put("nothing has been changed; add --confirm to go ahead".to_owned());
    }
    lines
}

/// What standing beside a setup already here came to, or would come to.
pub(super) fn beside(report: &BesideReport) -> Lines {
    if let Some(said) = turned_away(report.refusal.as_ref(), "not standing beside") {
        return said;
    }
    let mut lines = Lines::default();
    if report.stance == Stance::Applied {
        lines.put("lemonfiber now listens beside what was already here:".to_owned());
    } else {
        lines.put("standing beside what is here, lemonfiber would listen on:".to_owned());
    }
    for moved in &report.ports {
        lines.put(format!(
            "  {} — {} instead of {}",
            moved.service, moved.to, moved.from
        ));
    }
    lines.put(String::new());
    if let Some(written) = &report.written {
        lines.put(format!("written to {written}"));
    } else {
        lines.put("nothing has been written; add --confirm to go ahead".to_owned());
    }
    lines.put("nothing of the setup already here was touched".to_owned());
    lines
}

/// What standing in place of a setup already here came to, or would come to.
///
/// What is still up leads where anything is, because a half-stopped stack is the one
/// state an operator has to act on before they do anything else.
pub(super) fn replacement(report: &ReplaceReport) -> Lines {
    if let Some(said) = turned_away(report.refusal.as_ref(), "not standing in place of it") {
        return said;
    }
    let mut lines = Lines::default();
    let named = report.project.clone().unwrap_or_default();

    if !report.still_running.is_empty() {
        lines.put(format!("still running in {named}:"));
        for service in &report.still_running {
            lines.put(format!("  {service}"));
        }
        lines.put(String::new());
    }

    if report.stance == Stance::Applied {
        lines.put(format!("stopped in {named}:"));
        for service in &report.stopped {
            lines.put(format!("  {service}"));
        }
    } else {
        lines.put(format!("standing in place of {named} would stop:"));
        for service in &report.would_stop {
            lines.put(format!("  {service}"));
        }
    }

    lines.put(String::new());
    if report.stance == Stance::Applied {
        lines.put("nothing was deleted; start them again whenever you like".to_owned());
    } else {
        lines.put("nothing has been stopped; add --confirm to go ahead".to_owned());
    }
    lines
}

/// What copying an operator's own records across came to, or would come to.
///
/// What did not travel leads. A record still on the old stack and not on the new is the
/// thing an operator has to do something about; what arrived safely needs no action.
pub(super) fn carried(report: &ImportReport) -> Lines {
    if let Some(said) = turned_away(report.refusal.as_ref(), "not carrying anything across") {
        return said;
    }
    let mut lines = Lines::default();

    listed(&report.not_carried, "could not be carried:", &mut lines);

    let (records, heading) = if report.stance == Stance::Applied {
        (&report.carried, "carried across:")
    } else {
        (&report.would_carry, "would carry across:")
    };
    if !records.is_empty() {
        lines.put(String::new());
        lines.put(heading.to_owned());
        for record in records {
            lines.put(counted(record));
        }
    } else if report.stance == Stance::Unchanged {
        lines.put("nothing to carry across; the two hold the same records".to_owned());
    }

    lines.put(String::new());
    if report.stance == Stance::Applied {
        lines.put("the setup already here was only read from".to_owned());
    } else {
        lines.put("nothing has been carried; add --confirm to go ahead".to_owned());
    }
    lines
}

/// One record, as a line an operator reads.
fn counted(record: &RecordReport) -> String {
    format!("  {} — {} ({})", record.name, record.service, record.kind)
}

/// Every project already standing here, with what each service answers on.
fn standing_here(report: &MigrationReport, lines: &mut Lines) {
    if report.standing.is_empty() {
        lines.put("found no other setup on this machine".to_owned());
        return;
    }
    for project in &report.standing {
        lines.put(format!("{}:", project.project));
        for service in &project.services {
            lines.put(format!(
                "  {} — {}, {}",
                service.service,
                if service.running {
                    "running"
                } else {
                    "stopped"
                },
                published(&service.ports)
            ));
        }
    }
}

/// The ports a service answers on, or that it answers on none.
fn published(ports: &[u16]) -> String {
    if ports.is_empty() {
        return "no published port".to_owned();
    }
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<String>>()
        .join(", ")
}

/// Ports lemonfiber would want that something else already holds.
fn clashes(report: &MigrationReport, lines: &mut Lines) {
    if report.conflicts.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("ports lemonfiber would want that are already taken:".to_owned());
    for clash in &report.conflicts {
        lines.put(format!(
            "  {} — wanted by {}, held by {}",
            clash.port, clash.wanted_by, clash.held_by
        ));
    }
}

/// What the existing layout costs, where it cannot hold a hardlink.
///
/// The remedy is put beside the cost and marked as theirs to take. A layout that
/// breaks hardlinks is somebody's years of library sitting where they put it, and a
/// survey that read as an instruction would be telling them to move it.
fn layout(report: &MigrationReport, lines: &mut Lines) {
    let Some(linking) = &report.linking else {
        return;
    };
    lines.put(String::new());
    lines.put("this layout cannot hardlink:".to_owned());
    lines.put(format!("  {}", linking.because));
    lines.put(format!("  {}", linking.cost));
    lines.put(format!("  you could: {}", linking.remedy));
    lines.put("  lemonfiber will not move anything to do it".to_owned());
}

/// What taking each recognised service over would come to.
fn taking_over(report: &MigrationReport, lines: &mut Lines) {
    if report.carrying.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("what taking these over would come to:".to_owned());
    for service in &report.carrying {
        lines.put(format!(
            "  {} {} → {} — {}",
            service.service,
            service.existing,
            service.ours,
            cost(service.refused, service.backup_first)
        ));
        lines.put(format!("    {}", service.because));
    }
}

/// What taking one service over costs, in the two words that change what can be done.
///
/// The refusal outranks the backup: a service lemonfiber will not open is not one an
/// operator needs to be told to back up first.
const fn cost(refused: bool, backup_first: bool) -> &'static str {
    if refused {
        "will not"
    } else if backup_first {
        "backup first"
    } else {
        "as it stands"
    }
}

/// What may be done about what was found, and where a second copy would listen.
fn choices(report: &MigrationReport, lines: &mut Lines) {
    if !report.modes.is_empty() {
        lines.put(String::new());
        lines.put("what you can do about it:".to_owned());
        for mode in &report.modes {
            lines.put(format!(
                "  {}{}",
                mode.mode,
                marked(mode.preselected, mode.disturbs)
            ));
            lines.put(format!("    {}", mode.what));
        }
    }
    if report.beside.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put("running side-by-side, lemonfiber would listen on:".to_owned());
    for moved in &report.beside {
        lines.put(format!(
            "  {} — {} instead of {}",
            moved.service, moved.to, moved.from
        ));
    }
}

/// How a mode is marked in the list.
///
/// The default and the destructive one, because those are the two an operator has to
/// tell apart before reading any further.
const fn marked(preselected: bool, disturbs: bool) -> &'static str {
    if preselected {
        " (default)"
    } else if disturbs {
        " (stops what is running)"
    } else {
        ""
    }
}

/// One list of things found, under a heading, or nothing where there are none.
fn listed(items: &[UnsupportedReport], heading: &str, lines: &mut Lines) {
    if items.is_empty() {
        return;
    }
    lines.put(String::new());
    lines.put(heading.to_owned());
    for item in items {
        lines.put(format!("  {} — {}", item.what, item.because));
    }
}

#[cfg(test)]
mod tests;
