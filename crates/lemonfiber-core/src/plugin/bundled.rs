//! Whether what the bundled stack says each service provides holds against the
//! recordings the stack carries.
//!
//! The same judgement a plugin's claims meet, by the same reader and the same judge: a
//! bundled service gets no gentler treatment for being bundled. Each probe is judged
//! against its recording, which must have been taken from the image the stack pins, so
//! a pin moved without re-recording is refused rather than trusted.
//!
//! **Three answers, and only one of them refuses.** A recording that contradicts its
//! probe refutes the claim, and so does a claim the contract refuses outright. A probe
//! with no readable recording of its request, and a capability with no claim at all,
//! establish nothing either way: they are reported as unproven and never counted as
//! shown.

use std::path::Path;

use lemonfiber_manifest::Manifest;
use serde::Serialize;

use super::claimed::pinned_to;
use super::Verdict;
use lemonfiber_plugin::Violation;

/// One probe of one bundled claim, and what its recording came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Judged {
    /// The service that claims it.
    pub service: String,
    /// The capability the claim demonstrates.
    pub capability: String,
    /// The probe of that capability.
    pub probe: String,
    /// What the recording said about it.
    pub verdict: Verdict,
}

/// A capability a bundled service provides that no claim demonstrates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unclaimed {
    /// The service that says it provides it.
    pub service: String,
    /// What it says it provides.
    pub capability: String,
}

/// Every bundled claim, judged against the stack's recordings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Bundled {
    /// Every probe of every claim that could be read.
    pub judged: Vec<Judged>,
    /// Every capability provided and demonstrated by nothing.
    pub unclaimed: Vec<Unclaimed>,
    /// Everything the contract refuses: a claim it cannot read or will not bind, and a
    /// recording taken from an image the stack does not pin.
    pub refused: Vec<Violation>,
}

impl Bundled {
    /// The probes a recording contradicts.
    pub fn refuted(&self) -> impl Iterator<Item = &Judged> {
        self.judged
            .iter()
            .filter(|one| matches!(one.verdict, Verdict::Failed { .. }))
    }

    /// The probes nothing could be judged against.
    pub fn unproven(&self) -> impl Iterator<Item = &Judged> {
        self.judged
            .iter()
            .filter(|one| matches!(one.verdict, Verdict::Unproven { .. }))
    }

    /// Whether nothing refutes or refuses a bundled claim. Unproven is not a refusal.
    #[must_use]
    pub fn holds(&self) -> bool {
        self.refused.is_empty() && self.refuted().next().is_none()
    }
}

/// Judge every bundled claim in `manifest` against the recordings beneath `stack`, the
/// directory the manifest was read from.
#[must_use]
pub fn judged(manifest: &Manifest, stack: &Path) -> Bundled {
    let mut report = Bundled::default();
    for service in &manifest.services {
        let read = lemonfiber_plugin::claiming::bundled(service);
        report.refused.extend(read.violations);
        report
            .unclaimed
            .extend(read.unclaimed.into_iter().map(|capability| Unclaimed {
                service: service.id.clone(),
                capability,
            }));
        let pinned = service.reference();
        for claim in &read.claims {
            for probe in &claim.probes {
                let verdict = match pinned_to(
                    Some(&probe.fixture),
                    &probe.request,
                    &probe.expect,
                    &pinned,
                    stack,
                    &mut report.refused,
                ) {
                    Err(why) => Verdict::Unproven { why },
                    Ok(faults) if faults.is_empty() => Verdict::Passed,
                    Ok(faults) => Verdict::Failed {
                        faults: faults.into_iter().map(|fault| fault.said).collect(),
                    },
                };
                report.judged.push(Judged {
                    service: service.id.clone(),
                    capability: claim.capability.clone(),
                    probe: probe.id.clone(),
                    verdict,
                });
            }
        }
    }
    report
}

/// The report as lines, worst first: what refuses, then what refutes, then what is
/// unproven, then what was shown, and a count of each at the end.
#[must_use]
pub fn said(report: &Bundled) -> Vec<String> {
    let mut lines: Vec<String> = report
        .refused
        .iter()
        .map(|refused| format!("refused: {refused}"))
        .collect();
    let mut unproven = report.unclaimed.len();
    let (mut refuted, mut shown) = (0, 0);
    let mut later = Vec::new();
    for one in &report.judged {
        let about = format!("{} {} probe {}", one.service, one.capability, one.probe);
        match &one.verdict {
            Verdict::Failed { faults } => {
                refuted += 1;
                lines.push(format!("refuted: {about}: {}", faults.join("; ")));
            }
            Verdict::Unproven { why } => {
                unproven += 1;
                later.push(format!("unproven: {about}: {why}"));
            }
            Verdict::Passed | Verdict::FailingAsDeclared { .. } => {
                shown += 1;
                later.push(format!("demonstrated: {about}"));
            }
        }
    }
    later.sort_by_key(|line| !line.starts_with("unproven"));
    lines.extend(report.unclaimed.iter().map(|gap| {
        format!(
            "unproven: {} provides {}, and no claim demonstrates it",
            gap.service, gap.capability
        )
    }));
    lines.extend(later);
    lines.push(format!(
        "{} refused, {refuted} refuted, {unproven} unproven, {shown} demonstrated",
        report.refused.len()
    ));
    lines
}

#[cfg(test)]
mod tests;
