//! What the diagnostic checks found, finding by finding.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::doctor::{Overall, Verdict};
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::DoctorReport;

use super::Lines;

/// What the diagnostic checks found, finding by finding.
///
/// Each finding leads with a mark that reads at a glance and the plain evidence
/// behind it; a non-passing one carries the reason and what to do, because a
/// finding without a remedy is a fault report rather than a diagnosis.
pub(super) fn diagnosis(report: &DoctorReport) -> Lines {
    let mut lines = Lines::default();
    for finding in &report.findings {
        let title = &finding.title;
        match &finding.verdict {
            Verdict::Pass { note } => match note {
                Some(note) => lines.put(format!("  ✓ {title}   {note}")),
                None => lines.put(format!("  ✓ {title}")),
            },
            Verdict::Warn(problem) => {
                lines.put(format!("  ! {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Fail(problem) => {
                lines.put(format!("  ✗ {title}   {}", problem.summary));
                lines.extend(remedies(problem));
            }
            Verdict::Unverified { reason, remedy } => {
                lines.put(format!("  ? {title}   UNVERIFIED"));
                lines.put(format!("      {reason}"));
                lines.put(format!("      → {}", remedy.action));
                if let Some(detail) = &remedy.detail {
                    lines.put(format!("        {detail}"));
                }
            }
            Verdict::Skipped { reason } => {
                lines.put(format!("  – {title}   skipped: {reason}"));
            }
        }
    }

    lines.spaced(overall(report.overall));
    lines
}

/// The problem's meaning and remedies, indented under a finding.
pub(super) fn remedies(problem: &Problem) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("      {}", problem.meaning));
    for remedy in &problem.remedies {
        lines.put(format!("      → {}", remedy.action));
        if let Some(detail) = &remedy.detail {
            lines.put(format!("        {detail}"));
        }
    }
    lines
}

/// The one-line verdict a diagnosis amounts to.
pub(super) fn overall(overall: Overall) -> &'static str {
    match overall {
        Overall::Healthy => "healthy — everything checked passed",
        Overall::Degraded => "degraded — working, with warnings",
        Overall::Broken => "broken — something needs attention",
        Overall::Unknown => "unknown — health could not be established",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fixtures::*;
    use lemonfiber_core::doctor::{Category, Finding, Overall, Verdict};
    use lemonfiber_core::error::{Code, Problem, Remedy, Severity};

    #[test]
    fn every_verdict_reads_with_its_own_mark() {
        let findings = vec![
            Finding {
                check: "a".to_owned(),
                category: Category::Storage,
                title: "noted".to_owned(),
                verdict: Verdict::Pass {
                    note: Some("plenty of room".to_owned()),
                },
            },
            Finding {
                check: "b".to_owned(),
                category: Category::Storage,
                title: "bare".to_owned(),
                verdict: Verdict::Pass { note: None },
            },
            Finding {
                check: "c".to_owned(),
                category: Category::Vpn,
                title: "warned".to_owned(),
                verdict: Verdict::Warn(a_problem()),
            },
            Finding {
                check: "d".to_owned(),
                category: Category::Vpn,
                title: "failed".to_owned(),
                verdict: Verdict::Fail(a_problem()),
            },
            Finding {
                check: "e".to_owned(),
                category: Category::Network,
                title: "unproven".to_owned(),
                verdict: Verdict::Unverified {
                    reason: "nothing answered".to_owned(),
                    remedy: Remedy::new("start it").with_detail("compose up"),
                },
            },
            Finding {
                check: "f".to_owned(),
                category: Category::Network,
                title: "passed over".to_owned(),
                verdict: Verdict::Skipped {
                    reason: "not applicable".to_owned(),
                },
            },
        ];
        let report = DoctorReport {
            overall: Overall::Degraded,
            findings,
        };
        let text = diagnosis(&report).text();
        assert!(text.contains("✓ noted   plenty of room"));
        assert!(text.contains("✓ bare"));
        assert!(text.contains("! warned   it broke"));
        assert!(text.contains("✗ failed   it broke"));
        assert!(text.contains("? unproven   UNVERIFIED"));
        assert!(text.contains("→ start it"));
        assert!(text.contains("compose up"));
        assert!(text.contains("– passed over   skipped: not applicable"));
        assert!(text.contains("degraded — working, with warnings"));
    }

    #[test]
    fn an_unverified_finding_without_detail_still_carries_its_remedy() {
        let report = DoctorReport {
            overall: Overall::Unknown,
            findings: vec![Finding {
                check: "a".to_owned(),
                category: Category::Config,
                title: "unproven".to_owned(),
                verdict: Verdict::Unverified {
                    reason: "nothing answered".to_owned(),
                    remedy: Remedy::new("start it"),
                },
            }],
        };
        assert!(diagnosis(&report).text().contains("→ start it"));
    }

    #[test]
    fn a_remedy_without_detail_prints_only_its_action() {
        let problem = Problem::new(
            Code::new("TEST"),
            Severity::Warning,
            "it broke",
            "nothing imports",
            Remedy::new("restart it"),
        );
        let text = remedies(&problem).text();
        assert!(text.contains("nothing imports"));
        assert!(text.contains("→ restart it"));
    }

    #[test]
    fn every_overall_verdict_reads_as_a_sentence() {
        for verdict in [
            Overall::Healthy,
            Overall::Degraded,
            Overall::Broken,
            Overall::Unknown,
        ] {
            assert!(overall(verdict).contains('—'));
        }
    }
}
