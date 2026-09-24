//! The choices the operator has answered, kept between runs.
//!
//! Some of what this tool reports is not a fault but a decision — running torrents
//! with no VPN containing them is the one that matters. Stating what it costs is
//! right, once. Stating it on every run afterwards is the same sentence again, and
//! an operator who has weighed it learns the tool repeats itself — after which
//! they skim past the findings that are faults too.
//!
//! Only what the operator answered lives here. What running without a forwarded
//! port costs is not answered at all: it is said at the moment the choice is made
//! and never raised again, which [`super::unforwarded`] does without needing a record.
//!
//! Kept with configuration rather than beside the stack, because it is a record of
//! something the operator decided: a backup that restored the stack without it
//! would put every settled question to them again.
//!
//! Best-effort to read and strict to write, the same way the notification choice
//! is. A record that cannot be read means a choice is put again, which is
//! tiresome; one that cannot be written and says nothing would leave them
//! believing they had settled something they had not.

use std::path::PathBuf;

use crate::doctor::acknowledged::{suppressing, Accepted};
use crate::doctor::Verdict;
use crate::error::{Problem, Remedy, Severity};
use crate::model::DoctorReport;

use super::Ctx;

use crate::error::codes::ack::NOT_WARNED;

/// What the operator has answered, or nothing where they have answered nothing.
#[must_use]
pub(crate) fn load(ctx: &Ctx) -> Accepted {
    super::record::kept(path(ctx).as_deref())
}

/// Record it where the next run will find it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub(crate) fn save(ctx: &Ctx, accepted: &Accepted) -> Result<(), Box<Problem>> {
    super::record::keep(path(ctx).as_deref(), accepted)
}

/// Where the record is kept: beside the environment file, or nowhere on a machine
/// with nothing configured — which has no choices recorded either.
fn path(ctx: &Ctx) -> Option<PathBuf> {
    super::targets::beside_env(ctx, "accepted.json")
}

/// Record the operator's answer to one check, and report as though it had already
/// been given.
///
/// Only a check this very report is warning about can be answered. A name nothing
/// is warning about is a typo or a misremembering, and recording it would leave
/// the operator believing they had settled something — the tool would then go on
/// saying the thing they thought they had answered, and they would stop trusting
/// either half of it. A failure cannot be answered at all: it is not a choice.
///
/// A rehearsal is this run with the recording left out, and the report it gives is
/// the one a real answer produces: the check it names stops leading, in the same
/// findings, through the same suppression. That is the whole of what an operator is
/// deciding about — which warning would go quiet and what it was saying — and it is
/// reached without writing an answer down, so the question they asked does not settle
/// anything on their behalf. The refusal above still applies, because a name nothing
/// warns about is a mistake whether or not this run means it.
///
/// # Errors
///
/// Where the named check is not warning in this report, or the answer cannot be
/// written down.
pub(crate) fn acknowledge(
    ctx: &Ctx,
    accept: Option<&str>,
    report: DoctorReport,
) -> Result<DoctorReport, Box<Problem>> {
    let Some(check) = accept else {
        return Ok(report);
    };
    if !warns_about(&report, check) {
        return Err(Box::new(not_warned(&report, check)));
    }
    let mut answered = load(ctx);
    answered.accept(check);
    if !ctx.dry_run {
        save(ctx, &answered)?;
    }
    Ok(DoctorReport {
        findings: suppressing(report.findings, &answered),
        ..report
    })
}

/// Whether this report carries a warning about the named check.
fn warns_about(report: &DoctorReport, check: &str) -> bool {
    report
        .findings
        .iter()
        .any(|finding| finding.check == check && matches!(finding.verdict, Verdict::Warn(_)))
}

/// Why an answer was refused, naming what could be answered instead.
///
/// The alternatives come from the report rather than from a list kept here, so a
/// check that starts warning about a choice is offerable the day it does and one
/// that stops cannot be accepted into a record nothing reads.
fn not_warned(report: &DoctorReport, check: &str) -> Problem {
    let answerable: Vec<&str> = report
        .findings
        .iter()
        .filter(|finding| matches!(finding.verdict, Verdict::Warn(_)))
        .map(|finding| finding.check.as_str())
        .collect();
    let remedy = if answerable.is_empty() {
        Remedy::new("Run the checks first, and answer a warning they actually raise")
    } else {
        Remedy::new("Answer one of the warnings this run raised").with_detail(format!(
            "lemonfiber doctor --accept {}",
            answerable.join(" | ")
        ))
    };
    Problem::new(
        NOT_WARNED,
        Severity::Error,
        format!("Nothing in this run warns about {check}"),
        "An answer is only meaningful against something the tool is currently saying. \
         Recording one for anything else would leave a question settled that is still \
         being asked.",
        remedy,
    )
}

#[cfg(test)]
mod tests;
