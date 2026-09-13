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
            wiring(
                "l",
                SeedState::WouldWire {
                    yours: None,
                    ours: Some("/data/media/tv".to_owned()),
                },
            ),
            wiring(
                "m",
                SeedState::WouldWire {
                    yours: Some("tv-sonarr".to_owned()),
                    ours: Some("lemonfiber".to_owned()),
                },
            ),
            wiring(
                "n",
                SeedState::WouldWire {
                    yours: None,
                    ours: None,
                },
            ),
            wiring("o", SeedState::WouldAdopt),
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
            "would be set to “/data/media/tv”",
            "would be changed from “tv-sonarr” to “lemonfiber”",
            "would be set to a newly generated value",
            "would be adopted",
        ] {
            assert!(text.contains(phrase), "missing {phrase}");
        }
    }

    /// A rehearsal's last lines are the two an operator has to read: what a real run
    /// would still have to do, and that this one did none of it.
    #[test]
    fn a_rehearsed_pass_says_it_wrote_nothing_and_what_running_it_for_real_would_take() {
        let waiting = SeedReport {
            wirings: vec![wiring(
                "a",
                SeedState::WouldWire {
                    yours: None,
                    ours: Some("http://sonarr:8989".to_owned()),
                },
            )],
            assessment: SeedAssessment::Assessed,
            rehearsed: true,
        };
        let text = seeding(&waiting).text();
        assert!(text.contains("1 left to wire — run it again without --dry-run."));
        assert!(text.contains("Nothing was written."));
        assert!(
            !text.contains("run seed again once ready"),
            "a rehearsal that tells them to run it again has told them it ran: {text}"
        );

        let settled = SeedReport {
            wirings: vec![wiring("a", SeedState::AlreadyWired)],
            assessment: SeedAssessment::Assessed,
            rehearsed: true,
        };
        let done = seeding(&settled).text();
        assert!(done.contains("a real run would change nothing"), "{done}");
        assert!(done.contains("Nothing was written."), "{done}");
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
            rehearsed: false,
        };
        assert!(seeding(&report).text().contains("could not be read"));
    }
}
