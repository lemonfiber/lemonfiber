//! What seeding wired, connection by connection.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.

use lemonfiber_core::seed::{
    Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
    State as SeedState,
};
use lemonfiber_core::PRODUCT;

use super::Lines;

/// What seeding wired, connection by connection, with what a re-run still owes
/// named last so it is the thing the operator is left looking at.
pub(super) fn seeding(report: &SeedReport) -> Lines {
    let mut lines = Lines::default();
    for wiring in &report.wirings {
        let connection = &wiring.connection;
        match &wiring.state {
            SeedState::Wired => lines.put(format!("  ✓ {connection}   wired")),
            SeedState::AlreadyWired => lines.put(format!("  ✓ {connection}   already wired")),
            SeedState::Drifted => lines.put(format!("  · {connection}   left as you set it")),
            SeedState::Adopted => lines.put(format!("  ✓ {connection}   yours, adopted")),
            SeedState::Unmanaged => lines.put(format!(
                "  · {connection}   found already set — yours, left as is (run `{PRODUCT} adopt` to keep it)"
            )),
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
    }
    let warnings = report.warnings();
    if !warnings.is_empty() {
        lines.spaced(format!(
            "{} drifted in a way that breaks the stack — see the ! lines above.",
            warnings.len()
        ));
    }
    let outstanding = report.outstanding();
    let blocked = report.blocked();
    if outstanding.is_empty() {
        lines.spaced("Everything is wired.");
    } else if blocked.is_empty() {
        lines.spaced(format!(
            "{} left to wire — run seed again once ready.",
            outstanding.len()
        ));
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
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fixtures::*;
    use lemonfiber_core::seed::{
        Assessment as SeedAssessment, Report as SeedReport, Severity as SeedSeverity,
        State as SeedState, Wiring,
    };

    #[test]
    fn every_seed_state_says_what_became_of_the_connection() {
        let report = seed_report(vec![
            wiring("a", SeedState::Wired),
            wiring("b", SeedState::AlreadyWired),
            wiring("c", SeedState::Drifted),
            wiring("d", SeedState::Adopted),
            wiring("e", SeedState::Unmanaged),
            wiring("f", SeedState::Stale),
            wiring(
                "g",
                SeedState::Conflicted {
                    yours: Some("mine".to_owned()),
                    ours: "ours".to_owned(),
                },
            ),
            wiring(
                "h",
                SeedState::Conflicted {
                    yours: None,
                    ours: "ours".to_owned(),
                },
            ),
            wiring(
                "i",
                SeedState::Skipped {
                    reason: "not up".to_owned(),
                },
            ),
            wiring(
                "j",
                SeedState::Failed {
                    detail: "refused".to_owned(),
                },
            ),
            wiring(
                "k",
                SeedState::Refused {
                    reason: "two arrs".to_owned(),
                },
            ),
        ]);
        let text = seeding(&report).text();
        for phrase in [
            "wired",
            "already wired",
            "left as you set it",
            "yours, adopted",
            "found already set",
            "yours for now",
            "conflict — both you and the default changed it",
            "you set “mine”",
            "you cleared it",
            "skipped",
            "refused",
        ] {
            assert!(text.contains(phrase), "missing {phrase}");
        }
    }

    #[test]
    fn a_drift_that_breaks_the_stack_is_raised_beneath_its_line() {
        let report = seed_report(vec![Wiring {
            connection: "root folder".to_owned(),
            state: SeedState::Drifted,
            severity: SeedSeverity::Warning {
                breakage: "the path does not exist".to_owned(),
                remediation: "create it".to_owned(),
            },
        }]);
        let text = seeding(&report).text();
        assert!(text.contains("! the path does not exist"));
        assert!(text.contains("→ create it"));
        assert!(text.contains("1 drifted in a way that breaks the stack"));
    }

    #[test]
    fn what_a_seed_still_owes_is_the_last_thing_said() {
        // Everything settled.
        assert!(seeding(&seed_report(vec![wiring("a", SeedState::Wired)]))
            .text()
            .contains("Everything is wired."));
        // Outstanding but nothing blocked.
        let waiting = seed_report(vec![wiring(
            "a",
            SeedState::Skipped {
                reason: "not up".to_owned(),
            },
        )]);
        assert!(seeding(&waiting).text().contains("1 left to wire"));
        // Everything outstanding is blocked.
        let blocked = seed_report(vec![wiring(
            "a",
            SeedState::Refused {
                reason: "two arrs".to_owned(),
            },
        )]);
        assert!(seeding(&blocked).text().contains("1 to resolve"));
        // A mix of the two.
        let mixed = seed_report(vec![
            wiring(
                "a",
                SeedState::Refused {
                    reason: "two arrs".to_owned(),
                },
            ),
            wiring(
                "b",
                SeedState::Skipped {
                    reason: "not up".to_owned(),
                },
            ),
        ]);
        assert!(seeding(&mixed)
            .text()
            .contains("2 left: 1 to wire once ready"));
    }

    #[test]
    fn a_lost_baseline_says_drift_could_not_be_assessed() {
        let report = SeedReport {
            wirings: vec![wiring("a", SeedState::Wired)],
            assessment: SeedAssessment::Unassessable,
        };
        assert!(seeding(&report).text().contains("could not be read"));
    }
}
