//! Running the register, and what a run comes to.
//!
//! Apart from the vocabulary it works in, because the two change for different
//! reasons: what a check *is* and what a finding carries move when the report gains a
//! shape, and this moves when the way a run is bounded, narrowed or summed changes.
//!
//! Four places a run could quietly lie, and one answer to each. Which checks are
//! reached is [`Narrowing`]'s to decide rather than this module's; a check that does
//! not answer inside its budget becomes an unverified finding rather than a hung
//! suite; a finding that is only there because something underneath it failed says so;
//! and the word at the top is summed from what was kept rather than carried over from
//! what was run.

use std::collections::BTreeMap;

use super::{Check, Finding, Narrowing, Overall, Verdict};
use crate::error::Remedy;
use crate::model::DoctorReport;

/// Run the checks that match, bound each by its own budget, and sum the result.
///
/// Naming a family runs only that one, and naming a check reports only that one —
/// by the same id the finding carries, so what a report names is what can be asked
/// for again. Each check is awaited under a timeout, so a wedged command becomes an
/// unverified finding rather than a hung suite; because the checks are values in a
/// list, one timing out has no bearing on the next.
///
/// The overall verdict is summed from what was kept, so a run narrowed to one check
/// is graded on that check rather than on the family it happened to arrive with.
pub async fn examine(checks: &[Box<dyn Check>], narrowing: &Narrowing) -> DoctorReport {
    // The checks are independent I/O — process spawns, container execs, HTTP — so
    // they run at once rather than in series: a run's wall-clock then tracks the
    // slowest check, not their sum, which matters most on the struggling stack an
    // operator reaches for `doctor` against. `join_all` preserves the checks' order,
    // so the report reads the same as when they ran one at a time; each is still
    // bounded by its own budget, so one hanging has no bearing on the rest.
    let selected = checks.iter().filter(|check| narrowing.runs(check.as_ref()));
    let findings: Vec<Finding> = futures_util::future::join_all(selected.map(|check| async move {
        match tokio::time::timeout(check.budget(), check.run()).await {
            Ok(produced) => produced,
            Err(_elapsed) => vec![timed_out(check.as_ref())],
        }
    }))
    .await
    .into_iter()
    .flatten()
    .filter(|finding| narrowing.keeps(finding))
    .collect();
    DoctorReport {
        overall: overall(&findings),
        findings,
    }
}

/// Attribute each finding to the finding underneath it, where one explains another.
///
/// A stack whose VPN is down raises a finding for every service behind it, and a flat list
/// of them reads as a dozen unrelated faults rather than as one. What relates them is the
/// manifest's own `depends_on`: a service that cannot work because something it needs is
/// also in trouble is one problem with the thing underneath, not two.
///
/// The relationship is read from the manifest rather than from a table of check ids here,
/// for the same reason narrowing reads `profile.protocol` rather than recognising profile
/// names — a stack that renames or rewires its services keeps working, and nothing in this
/// crate has to know what any service is.
///
/// Only a finding that is not passing can explain another: a dependency that is working
/// explains nothing about the service on top of it. Findings about nothing in particular —
/// the environment, the filesystem — are left alone, having no service to be downstream of.
#[must_use]
pub fn attributed(
    findings: Vec<Finding>,
    services: &[lemonfiber_manifest::Service],
) -> Vec<Finding> {
    let troubled: BTreeMap<&str, &str> = findings
        .iter()
        .filter(|finding| !matches!(finding.verdict, Verdict::Pass { .. }))
        .filter_map(|finding| Some((finding.service.as_deref()?, finding.check.as_str())))
        .collect();

    let attributed: Vec<Option<String>> = findings
        .iter()
        .map(|finding| {
            finding
                .service
                .as_deref()
                .and_then(|service| services.iter().find(|had| had.id == service))
                .into_iter()
                .flat_map(|service| service.depends_on.iter())
                .find_map(|needed| troubled.get(needed.as_str()).map(|&check| check.to_owned()))
        })
        .collect();

    findings
        .into_iter()
        .zip(attributed)
        .map(|(finding, cause)| Finding {
            caused_by: cause,
            ..finding
        })
        .collect()
}

/// The finding a check becomes when it does not answer within its budget.
///
/// Reported against what the check says it reports against, and against its family
/// only where it says nothing — which is a check that would have produced several
/// findings and cannot name one of them before running. Naming it matters most for
/// the checks that are not lemonfiber's own: a contributed check abandoned at its
/// budget has to be reported as that plugin's check having not run, and a finding
/// carrying its family alone would leave nothing on the page to attribute.
fn timed_out(check: &dyn Check) -> Finding {
    let seconds = check.budget().as_secs();
    let reported = check.reports();
    Finding {
        check: reported.as_ref().map_or_else(
            || check.category().as_str().to_owned(),
            |one| one.check.clone(),
        ),
        category: check.category(),
        service: reported.as_ref().and_then(|one| one.service.clone()),
        caused_by: None,
        said: None,
        title: "Check timed out".to_owned(),
        verdict: Verdict::Unverified {
            reason: format!("did not finish within {seconds} seconds"),
            remedy: Remedy::new(
                "Run it again; if it keeps timing out, the engine or a service is not responding",
            ),
        },
        origin: reported
            .as_ref()
            .map_or(crate::origin::Origin::Bundled, |one| one.origin.clone()),
    }
}

/// Reduce findings to one word.
///
/// Public as well as used here, because a caller that keeps only some of a run's
/// findings has to be able to sum what it kept. Re-summing is the whole point: a
/// verdict carried over from the run a finding was dropped from would be a word about
/// findings that are no longer there.
///
/// Highest consequence wins, and it is never an average — one leaking VPN among
/// eighteen passes is not eighteen-nineteenths healthy. An unverified finding
/// keeps the answer out of `healthy`: a thing that could not be checked is not a
/// thing that passed, and a run that established nothing is `unknown` rather than
/// a comfortable green.
#[must_use]
pub fn overall(findings: &[Finding]) -> Overall {
    let mut passed = false;
    let mut warned = false;
    let mut unverified = false;
    for finding in findings {
        match finding.verdict {
            Verdict::Fail(_) => return Overall::Broken,
            Verdict::Warn(_) => warned = true,
            Verdict::Unverified { .. } => unverified = true,
            Verdict::Pass { .. } => passed = true,
            Verdict::Skipped { .. } => {}
        }
    }
    if warned {
        Overall::Degraded
    } else if unverified {
        Overall::Unknown
    } else if passed {
        Overall::Healthy
    } else {
        Overall::Unknown
    }
}

#[cfg(test)]
mod tests;
