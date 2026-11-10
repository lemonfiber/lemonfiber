//! What became of the last restart of this machine, as `lemonfiber boot` says it.

use lemonfiber_core::model::{BootOutcome, BootReport, BootTunnel, BootVerification};

use super::news::moment;
use super::Lines;

/// The last restart, the verification after it or why there was none, the newest
/// verification where it was after an earlier restart, and how long the stack has failed
/// to come back.
pub(super) fn booted(report: &BootReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(report.booted_at.map_or_else(
        || "This machine does not say when it last started.".to_owned(),
        |at| format!("This machine last started at {}.", moment(at)),
    ));
    if let Some(verification) = &report.verified {
        verified(&mut lines, "After it", verification);
    } else {
        lines.put("No verification has run since it started.");
        if let Some(declined) = &report.declined {
            lines.put(format!("  Nothing was started: {declined}"));
        }
        if let Some(last) = &report.last_verified {
            let started = last
                .against
                .map_or_else(|| "an earlier start".to_owned(), moment);
            let after = format!("After {started}");
            verified(&mut lines, &after, last);
        }
    }
    if let (Some(since), Some(boots)) = (report.failing_since, report.failed_boots) {
        lines.spaced(format!(
            "The stack has not come back since {}, across {boots} start(s).",
            moment(since)
        ));
    }
    lines
}

/// One verification, opened by `after`.
fn verified(lines: &mut Lines, after: &str, verification: &BootVerification) {
    lines.spaced(format!(
        "{after}, at {}, the stack {}.",
        moment(verification.at),
        match verification.outcome {
            BootOutcome::Passed => "came back",
            BootOutcome::Failed => "did not come back",
            BootOutcome::WaitingForInstall => "came back with a part waiting for its install",
        }
    ));
    if !verification.not_back.is_empty() {
        lines.put(format!("  not back   {}", verification.not_back.join(", ")));
    }
    lines.put(format!(
        "  tunnel     {}",
        match &verification.tunnel {
            BootTunnel::Held => "held".to_owned(),
            BootTunnel::NotEstablished { said } => format!("not established: {said}"),
            BootTunnel::NotChecked => "not checked".to_owned(),
        }
    ));
    if let Some(port) = &verification.port {
        let spelled =
            |port: Option<u16>| port.map_or_else(|| "none".to_owned(), |one| one.to_string());
        lines.put(format!(
            "  port       {} before, {} now, {}",
            spelled(port.was),
            spelled(port.now),
            if port.pushed {
                "every client takes it"
            } else {
                "not every client takes it"
            }
        ));
    }
    if let Some(said) = &verification.said {
        lines.put(format!("  {said}"));
    }
}

#[cfg(test)]
mod tests;
