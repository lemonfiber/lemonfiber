//! What seeding wired, connection by connection.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::model::UnsupportedReport;
use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState,
};
use lemonfiber_core::PRODUCT;

use super::Lines;

/// One connection's lines: what became of it, and the breakage beneath it where
/// the drift broke the stack.
///
/// Split out of the pass above because that one outgrew the length rule, and this
/// is the seam it was already written along: everything here is about one wiring,
/// and everything left there is about the pass as a whole.
fn connection(wiring: &lemonfiber_core::seed::Wiring) -> Lines {
    let mut lines = Lines::default();
    let connection = &wiring.connection;
    match &wiring.state {
        SeedState::Wired => lines.put(format!("  ✓ {connection}   wired")),
        SeedState::AlreadyWired => lines.put(format!("  ✓ {connection}   already wired")),
        SeedState::Drifted => lines.put(format!("  · {connection}   left as you set it")),
        SeedState::Adopted => lines.put(format!("  ✓ {connection}   yours, adopted")),
        SeedState::Unmanaged => lines.put(format!(
            "  · {connection}   found already set — yours, left as is (run `{PRODUCT} adopt` to keep it)"
        )),
        SeedState::Observed { reason } => {
            lines.put(format!(
                "  · {connection}   yours — you declared it unmanaged, so nothing was written"
            ));
            lines.put(format!("      {reason}"));
        }
        SeedState::Stale => lines.put(format!(
            "  · {connection}   yours for now — a newer default is not yet applied"
        )),
        SeedState::Conflicted { yours, ours } => {
            lines.put(format!(
                "  ✗ {connection}   conflict — both you and the default changed it"
            ));
            match yours {
                Some(yours) => lines.put(format!(
                    "      you set “{yours}”, the default is now “{ours}” — left as you set it"
                )),
                None => lines.put(format!(
                    "      you cleared it, the default is now “{ours}” — left as you set it"
                )),
            }
        }
        SeedState::WouldWire { yours, ours } => lines.put(format!(
            "  → {connection}   {}",
            would(yours.as_deref(), ours.as_deref())
        )),
        SeedState::WouldAdopt => lines.put(format!(
            "  → {connection}   found already set — yours, would be adopted"
        )),
        SeedState::Skipped { reason } => {
            lines.put(format!("  ? {connection}   skipped"));
            lines.put(format!("      {reason}"));
        }
        SeedState::Failed { detail } => {
            lines.put(format!("  ✗ {connection}   {detail}"));
        }
        SeedState::Refused { reason } => {
            lines.put(format!("  ✗ {connection}   refused"));
            lines.put(format!("      {reason}"));
        }
    }
    // A drift that broke the stack is raised beneath the line it sits on, naming
    // what broke and the fix — the warning severity a plain drift never carries.
    if let SeedSeverity::Warning {
        breakage,
        remediation,
    } = &wiring.severity
    {
        lines.put(format!("      ! {breakage}"));
        lines.put(format!("        → {remediation}"));
    }
    lines
}

/// What seeding wired, connection by connection, with what a re-run still owes
/// named last so it is the thing the operator is left looking at.
pub(super) fn seeding(report: &SeedReport) -> Lines {
    let mut lines = Lines::default();
    for wiring in &report.wirings {
        lines.extend(connection(wiring));
    }
    lines.extend(unwirable(&report.unsupported));
    let warnings = report.warnings();
    if !warnings.is_empty() {
        lines.spaced(format!(
            "{} drifted in a way that breaks the stack — see the ! lines above.",
            warnings.len()
        ));
    }
    let outstanding = report.outstanding();
    let blocked = report.blocked();
    // What the operator is told to do next, which is the one sentence that differs
    // between the two tenses: a pass that wired things is run again once the rest is
    // ready, and a pass that said what it would do is run for real.
    let again = if report.rehearsed {
        "run it again without --dry-run"
    } else {
        "run seed again once ready"
    };
    if outstanding.is_empty() {
        lines.spaced(if report.rehearsed {
            "Everything is already wired — a real run would change nothing."
        } else {
            "Everything is wired."
        });
    } else if blocked.is_empty() {
        lines.spaced(format!("{} left to wire — {again}.", outstanding.len()));
    } else if blocked.len() == outstanding.len() {
        lines.spaced(format!(
            "{} to resolve — settle the conflict, then seed again.",
            blocked.len()
        ));
    } else {
        lines.spaced(format!(
            "{} left: {} to wire once ready, {} to resolve — settle the conflict first.",
            outstanding.len(),
            outstanding.len() - blocked.len(),
            blocked.len(),
        ));
    }
    if matches!(report.assessment, SeedAssessment::Unassessable) {
        lines.spaced(
            "The record of what lemonfiber last wrote could not be read, so drift \
             could not be assessed this run. Run `lemonfiber adopt` to re-baseline \
             from the current state.",
        );
    }
    // Last, and said whatever the lines above came to. A report an operator reads as a
    // run is the failure this whole flag exists to prevent, and the one place they are
    // certain to have reached is the bottom.
    if report.rehearsed {
        lines.spaced(
            "Nothing was written. No service was asked to change anything, and the record \
             of what lemonfiber last wrote is as it was.",
        );
    }
    lines
}

/// What a rehearsal says about one connection, in the tense it is true in.
///
/// Three readings of one pair, because what the operator is looking at differs: a
/// connection that is not there would be made, one holding something else would be
/// moved off it, and one whose value a real run would generate can be described but
/// never shown — there is no value to show, on purpose.
fn would(yours: Option<&str>, ours: Option<&str>) -> String {
    match (yours, ours) {
        (_, None) => "would be set to a newly generated value".to_owned(),
        (None, Some(ours)) => format!("would be set to “{ours}”"),
        (Some(yours), Some(ours)) => format!("would be changed from “{yours}” to “{ours}”"),
    }
}

/// The services a pass could not wire because it cannot speak to them, under the
/// connections it did attempt.
///
/// Under rather than among, because these are not connections: nothing was tried, so
/// there is no outcome to report beside the others. What there is is a reason, and an
/// operator who wrote the declaration this is about is the only person who can act on
/// it.
fn unwirable(unsupported: &[UnsupportedReport]) -> Lines {
    let mut lines = Lines::default();
    if unsupported.is_empty() {
        return lines;
    }
    lines.spaced("These were not wired, because lemonfiber cannot speak to them:");
    for one in unsupported {
        lines.put(format!("  {} — {}", one.what, one.because));
    }
    lines
}

#[cfg(test)]
mod tests;
