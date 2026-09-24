//! Where this copy of lemonfiber stands, on a terminal.
//!
//! The standing leads, because it is the whole answer for most operators: this is the
//! newest one, or it is not, or nobody could tell. Underneath it goes the provenance —
//! the version, the file it was run from, and which tool put it there — which is what
//! a bug report is unactionable without.
//!
//! The command comes last and alone, because it is the line somebody copies. Where
//! there is no command there is a sentence saying which reason that is, since an
//! operator told only that a tool owns this copy has been given a fact rather than a
//! way forward, and will reach for the thing that overwrites the file.

use lemonfiber_core::model::UpdateReport;
use lemonfiber_core::self_update::Standing;

use super::Lines;

/// Where this copy stands, and what moving it would come to.
pub(crate) fn standing(report: &UpdateReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(headline(report));
    lines.extend(brought(report));
    lines.extend(provenance(report));
    if let Some(untold) = &report.untold {
        lines.spaced(untold.clone());
    }
    lines.extend(moving(report));
    lines.spaced(report.afterwards.clone());
    lines.spaced(report.carries.clone());
    lines
}

/// The one sentence an operator who reads no further is owed.
fn headline(report: &UpdateReport) -> String {
    match report.standing {
        Standing::Current => format!("lemonfiber {} is the newest released.", report.running),
        Standing::UpdateAvailable | Standing::ManagedExternally => {
            report.offered.as_ref().map_or_else(
                || format!("lemonfiber {} — a newer version exists.", report.running),
                |offered| {
                    format!(
                        "lemonfiber {} — {offered} has been released.",
                        report.running
                    )
                },
            )
        }
        Standing::CheckFailed => format!(
            "lemonfiber {} — whether anything newer exists could not be told.",
            report.running
        ),
    }
}

/// What the version on offer changed, where the check read it.
///
/// Above the provenance rather than below it, because it is the thing the operator is
/// deciding on. Where this copy is and what put it there answers the question after —
/// how to take the update — and is no use to somebody who has not decided to.
///
/// Nothing where the check read nothing: a release published before the notes came
/// back has none, and a machine that has never reached the address has not read any.
/// Saying so is [`UpdateReport::untold`]'s job and this stays quiet rather than
/// printing a heading over an empty space.
fn brought(report: &UpdateReport) -> Lines {
    let mut lines = Lines::default();
    let (Some(changed), Some(offered)) = (&report.changed, &report.offered) else {
        return lines;
    };
    lines.spaced(format!("What {offered} changed:"));
    lines.extend(super::changelog::flattened(changed));
    lines
}

/// Which file was run, and what put it there.
fn provenance(report: &UpdateReport) -> Lines {
    let mut lines = Lines::default();
    match &report.at {
        Some(at) => lines.spaced(format!("  run from  {at}")),
        None => lines.spaced("  run from  this machine would not say"),
    }
    match &report.owner {
        Some(owner) => lines.put(format!("  put here  by {owner}, which owns it")),
        None => lines.put(format!("  put here  {}", unowned(report))),
    }
    if report.replaceable == Some(false) {
        lines.put("  and       that directory will not take a new file, so replacing it here");
        lines.put("            needs whoever owns it — this will not try to become them");
    }
    lines
}

/// How a copy nobody owns got here, in the words the report uses.
fn unowned(report: &UpdateReport) -> &'static str {
    match report.installed {
        lemonfiber_core::self_update::Installed::Installer => "by the shell installer",
        lemonfiber_core::self_update::Installed::Elsewhere => "by hand — no tool owns it",
        _ => "not known",
    }
}

/// The line to copy, or the reason there is none.
fn moving(report: &UpdateReport) -> Lines {
    let mut lines = Lines::default();
    if let Some(configuration) = &report.configuration {
        lines.spaced(configuration.clone());
    }
    if let Some(command) = &report.command {
        lines.spaced(heading(report));
        lines.put(format!("  {command}"));
    } else if let Some(instead) = &report.instead {
        lines.spaced(instead.clone());
    }
    lines
}

/// What the command underneath is for.
fn heading(report: &UpdateReport) -> String {
    report.asked.as_ref().map_or_else(
        || "To take it:".to_owned(),
        |asked| format!("To move to {asked}:"),
    )
}

#[cfg(test)]
mod tests;
