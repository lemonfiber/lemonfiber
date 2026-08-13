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
//! and never raised again, which [`super::seeding`] does without needing a record.
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

use crate::config::store;
use crate::doctor::acknowledged::{suppressing, Accepted};
use crate::doctor::Verdict;
use crate::error::{Code, Diagnose, Problem, Remedy, Severity};
use crate::model::DoctorReport;

use super::Ctx;

/// Raised when an answer names something nothing is warning about.
const NOT_WARNED: Code = Code::new("ACK-1");

/// What the operator has answered, or nothing where they have answered nothing.
#[must_use]
pub fn load(ctx: &Ctx) -> Accepted {
    path(ctx)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Record it where the next run will find it.
///
/// # Errors
///
/// Where there is nowhere configured to keep it, or the file cannot be written.
pub fn save(ctx: &Ctx, accepted: &Accepted) -> Result<(), Box<Problem>> {
    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::write(&path, &serde_json::to_string(accepted).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
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
/// # Errors
///
/// Where the named check is not warning in this report, or the answer cannot be
/// written down.
pub fn acknowledge(
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
    save(ctx, &answered)?;
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
mod tests {
    use super::{load, save};
    use crate::doctor::acknowledged::Accepted;

    /// Where a test's scratch record lives. Naming it does not touch it.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lemonfiber-accepted-{}-{name}", std::process::id()))
    }

    /// A context whose environment file is in an emptied scratch directory.
    fn ctx_at(name: &str) -> crate::app::Ctx {
        let dir = scratch(name);
        let _ = std::fs::remove_dir_all(&dir);
        ctx_with(Some(dir.join(".env")))
    }

    /// A context with the given environment file, or none at all.
    fn ctx_with(env_file: Option<std::path::PathBuf>) -> crate::app::Ctx {
        crate::app::Ctx::new(
            std::sync::Arc::new(crate::test_support::Scripted(Ok(
                crate::test_support::spoke(""),
            ))),
            std::sync::Arc::new(crate::test_support::Reporting::absent()),
            std::sync::Arc::new(crate::adapters::System),
            std::sync::Arc::new(crate::adapters::Disk),
            crate::test_support::stack(),
            crate::config::Settings {
                env_file,
                ..crate::config::Settings::default()
            },
            crate::platform::Environment::MacOs,
        )
    }

    #[test]
    fn a_choice_answered_once_stays_answered() {
        // The whole point: a settled question is not put again next run.
        let ctx = ctx_at("round-trip");
        let mut accepted = Accepted::new();
        accepted.accept("vpn.tunnel");
        assert!(save(&ctx, &accepted).is_ok());

        let read_back = load(&ctx);
        assert!(read_back.has("vpn.tunnel"));
        assert!(!read_back.has("vpn.port-forward"), "and only that one");
    }

    #[test]
    fn a_machine_where_nothing_was_answered_starts_with_nothing() {
        assert_eq!(load(&ctx_at("fresh")), Accepted::new());
    }

    #[test]
    fn a_record_that_will_not_parse_puts_the_question_again() {
        // Tiresome, and the safe direction: the alternative is treating an
        // unreadable file as blanket consent.
        let ctx = ctx_at("corrupt");
        let mut accepted = Accepted::new();
        accepted.accept("vpn.tunnel");
        assert!(save(&ctx, &accepted).is_ok());
        let written = scratch("corrupt").join("accepted.json");
        assert!(
            written.exists(),
            "the record was written in the first place"
        );
        assert!(
            crate::config::store::write(&written, "not json at all").is_ok(),
            "and is then replaced with something unparsable"
        );
        assert_eq!(load(&ctx), Accepted::new());
    }

    #[test]
    fn a_record_that_cannot_be_written_is_reported_rather_than_swallowed() {
        // Somewhere to keep it and still no way to write it: a directory sits
        // where the file must go. Telling the operator a choice was settled when
        // it was not is the failure worth avoiding here.
        let ctx = ctx_at("blocked");
        let blocked = scratch("blocked").join("accepted.json");
        assert!(
            std::fs::create_dir_all(&blocked).is_ok(),
            "the blocking directory"
        );
        assert!(save(&ctx, &Accepted::new()).is_err());
    }

    #[test]
    fn a_record_with_nowhere_to_go_is_reported_rather_than_swallowed() {
        let ctx = ctx_with(None);
        assert!(save(&ctx, &Accepted::new()).is_err());
        assert_eq!(load(&ctx), Accepted::new());
    }

    /// A report carrying one finding.
    fn reporting(finding: crate::doctor::Finding) -> crate::model::DoctorReport {
        crate::model::DoctorReport {
            overall: crate::doctor::Overall::Degraded,
            findings: vec![finding],
        }
    }

    /// A warning about a choice, on the given check.
    fn warning(check: &str) -> crate::doctor::Finding {
        crate::doctor::Finding::in_category(
            crate::doctor::Category::Vpn,
            check,
            "Torrent traffic is contained",
            crate::doctor::Verdict::Warn(crate::error::Problem::new(
                crate::error::Code::new("VPN-8"),
                crate::error::Severity::Warning,
                "Torrent traffic is not contained by a VPN",
                "It leaves under this connection's own address.",
                crate::error::Remedy::new("Put the client behind a VPN container"),
            )),
        )
    }

    #[test]
    fn answering_a_warning_settles_it_in_this_run_and_the_next() {
        let ctx = ctx_at("acknowledge");
        let mut report = reporting(warning("vpn.unprotected"));
        // A passing check alongside it, because an answer to one finding must not
        // reach into the rest of the run.
        report.findings.push(crate::doctor::Finding::in_category(
            crate::doctor::Category::Vpn,
            "vpn.egress-match",
            "The tunnel",
            crate::doctor::Verdict::Pass { note: None },
        ));
        let answered = super::acknowledge(&ctx, Some("vpn.unprotected"), report);
        let states: Vec<crate::error::State> = answered
            .as_ref()
            .map(|report| {
                report
                    .findings
                    .iter()
                    .filter_map(|finding| match &finding.verdict {
                        crate::doctor::Verdict::Warn(problem)
                        | crate::doctor::Verdict::Fail(problem) => Some(problem.state),
                        crate::doctor::Verdict::Pass { .. }
                        | crate::doctor::Verdict::Unverified { .. }
                        | crate::doctor::Verdict::Skipped { .. } => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Settled in the run that answered it, rather than only from the next one:
        // an operator who answers and sees the warning again assumes it did nothing.
        assert_eq!(states, vec![crate::error::State::Suppressed]);
        assert!(load(&ctx).has("vpn.unprotected"), "and it is written down");
    }

    #[test]
    fn answering_something_nothing_warns_about_is_refused_rather_than_recorded() {
        // A typo recorded silently leaves the operator believing a question is
        // settled while the tool goes on asking it.
        let ctx = ctx_at("typo");
        let refused = super::acknowledge(
            &ctx,
            Some("vpn.unprotexted"),
            reporting(warning("vpn.unprotected")),
        );
        assert!(refused.is_err());
        assert!(!load(&ctx).has("vpn.unprotexted"), "nothing was written");
        let detail = refused.err().and_then(|problem| {
            problem
                .remedies
                .first()
                .and_then(|remedy| remedy.detail.clone())
        });
        assert_eq!(
            detail,
            Some("lemonfiber doctor --accept vpn.unprotected".to_owned()),
            "and it names what could be answered instead"
        );
    }

    #[test]
    fn a_failure_cannot_be_answered_at_all() {
        // The most damaging thing this could do, and the reason the check is on the
        // verdict rather than on the check name: a fault is not a choice.
        let ctx = ctx_at("failure");
        let failing = crate::doctor::Finding::in_category(
            crate::doctor::Category::Vpn,
            "vpn.egress-match",
            "The tunnel",
            crate::doctor::Verdict::Fail(crate::error::Problem::new(
                crate::error::Code::new("VPN-1"),
                crate::error::Severity::Critical,
                "Traffic is leaving outside the tunnel",
                "Every torrent is visible under this machine's own address.",
                crate::error::Remedy::new("Stop the download client"),
            )),
        );
        assert!(super::acknowledge(&ctx, Some("vpn.egress-match"), reporting(failing)).is_err());
        assert!(!load(&ctx).has("vpn.egress-match"));
    }

    #[test]
    fn a_run_that_answers_nothing_is_left_exactly_as_it_came() {
        let ctx = ctx_at("untouched");
        let report = reporting(warning("vpn.unprotected"));
        let same = super::acknowledge(&ctx, None, report.clone());
        assert_eq!(same.ok(), Some(report));
    }

    #[test]
    fn a_refusal_with_nothing_answerable_says_so_rather_than_offering_an_empty_list() {
        let ctx = ctx_at("nothing-answerable");
        let quiet = crate::model::DoctorReport {
            overall: crate::doctor::Overall::Healthy,
            findings: Vec::new(),
        };
        let refused = super::acknowledge(&ctx, Some("vpn.unprotected"), quiet);
        assert_eq!(
            refused.err().and_then(|problem| problem
                .remedies
                .first()
                .and_then(|remedy| remedy.detail.clone())),
            None,
            "an empty list would read as though nothing could ever be answered"
        );
    }
}
