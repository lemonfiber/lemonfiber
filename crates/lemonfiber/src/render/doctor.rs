//! What the diagnostic checks found, finding by finding.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::doctor::{Overall, Verdict};
use lemonfiber_core::error::{Problem, State};
use lemonfiber_core::model::DoctorReport;
use lemonfiber_core::repair::{mendable, ASK_FOR_REPAIRS};

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
            // An answered choice keeps its line and loses its lead: still there,
            // still saying what it costs, no longer repeating the remedy for
            // something the operator has already decided against doing.
            Verdict::Warn(problem) if problem.state == State::Suppressed => {
                lines.put(format!("  · {title}   {} (answered)", problem.summary));
            }
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
    // Which of the three kinds this is, said where it changes what the operator can do.
    // The other two already say so in their own remedies — one names where to go and the
    // other offers the bundle — and only this one has an answer they would not guess at.
    if mendable(problem.state) {
        lines.put("      lemonfiber can put this one right for you".to_owned());
        lines.put(format!("        {ASK_FOR_REPAIRS}"));
    }
    for remedy in &problem.remedies {
        lines.remedy(remedy, "      ");
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

    /// The classification an operator can act on. A finding lemonfiber can put right
    /// itself says so and names the command; the ones only they can act on already say
    /// where to go in their own remedies, so a label there would be noise.
    #[test]
    fn a_finding_lemonfiber_can_mend_says_so_and_names_the_command() {
        use lemonfiber_core::error::State;

        let mendable = remedies(&a_problem().in_state(State::Remediable)).text();
        assert!(
            mendable.contains("can put this one right for you"),
            "{mendable}"
        );
        assert!(mendable.contains("lemonfiber doctor --fix"), "{mendable}");

        let theirs = remedies(&a_problem()).text();
        assert!(!theirs.contains("can put this one right"), "{theirs}");
    }

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

    #[test]
    fn an_answered_choice_stops_leading_without_disappearing() {
        // The whole point of suppressing rather than removing: it is still on
        // screen, still saying what it costs, and no longer putting a remedy for
        // something the operator has already decided against.
        let report = DoctorReport {
            overall: Overall::Degraded,
            findings: vec![Finding {
                check: "vpn.unprotected".to_owned(),
                category: Category::Vpn,
                title: "Torrent traffic is contained".to_owned(),
                verdict: Verdict::Warn(a_problem().in_state(State::Suppressed)),
            }],
        };
        let text = diagnosis(&report).text();
        assert!(text.contains("· Torrent traffic is contained   it broke (answered)"));
        assert!(!text.contains("→ "), "the remedy is not put again: {text}");
    }
}
