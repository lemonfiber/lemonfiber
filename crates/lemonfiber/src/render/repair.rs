//! What a repairing run offered, and what became of it.
//!
//! The outcome leads every line, because that is the question being answered: not whether
//! lemonfiber ran something, but whether the fault is gone.

use lemonfiber_core::app::repair::Report;
use lemonfiber_core::journal::{Action, Undo};
use lemonfiber_core::repair::{Outcome, ASK_FOR_REPAIRS};

use super::Lines;

/// What was offered and what was done about it.
///
/// For a person only. What a parser reads is the envelope every other answer is
/// rendered into, which is the one both surfaces put on the wire.
pub(crate) fn mended(report: &Report) -> Lines {
    let mut lines = Lines::default();
    // Said first, because it is the one thing here the operator has to act on themselves.
    for beyond in &report.beyond {
        lines.put(format!(
            "{} has outlasted every repair for it.",
            beyond.check
        ));
        lines.remedy(&beyond.remedy, "  ");
    }
    if report.offered.is_empty() && report.beyond.is_empty() {
        lines.put("There is nothing here lemonfiber can put right itself.");
        return lines;
    }
    if report.offered.is_empty() {
        return lines;
    }
    if !report.acted {
        lines.put("These could be put right:");
        for repair in &report.offered {
            lines.put(format!("  {}", repair.does));
        }
        lines.spaced(format!(
            "Nothing has been changed. Run `{ASK_FOR_REPAIRS}` to be asked about each."
        ));
        return lines;
    }
    for done in &report.mended {
        lines.put(format!("{} — {}", said(&done.outcome), done.repair.does));
    }
    lines
}

/// What was put back, or that there was nothing to put back.
pub(crate) fn reversed(undos: &[Undo]) -> Lines {
    let mut lines = Lines::default();
    if undos.is_empty() {
        lines.put("There is no repair to put back.");
        return lines;
    }
    lines.put("Put back what the last repair changed:");
    for undo in undos {
        lines.put(format!("  {} — {}", undo.target, restoring(&undo.action)));
    }
    lines
}

/// What one reversal did, in the words of the thing it acted on.
fn restoring(action: &Action) -> String {
    match action {
        Action::Restore {
            key,
            value: Some(value),
            ..
        } => format!("{key} back to {value}"),
        // Nothing was there before, so putting it back means taking it away again.
        Action::Restore {
            key, value: None, ..
        } => format!("{key} removed, as it was"),
        Action::Remove { resource, id } => format!("{resource} {id} removed"),
        Action::Reconfigure {
            resource,
            field,
            value: Some(value),
            ..
        } => format!("{resource}'s {field} back to {value}"),
        // Nothing was there before, so putting it back means clearing it again.
        Action::Reconfigure {
            resource, field, ..
        } => format!("{resource}'s {field} cleared, as it was"),
        Action::Delete { path } => format!("{path} removed"),
    }
}

/// How one outcome reads.
///
/// A repair that ran and left the fault standing says so plainly rather than borrowing the
/// word for one that worked: the difference is the entire point of asking the check again,
/// and a report that blurred it would undo the care taken to establish it.
fn said(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Fixed => "fixed".to_owned(),
        Outcome::FixFailed => "still wrong afterwards".to_owned(),
        // The state it was left in is the most useful sentence in a failed repair, so it
        // travels with the verdict rather than being flattened out of it.
        Outcome::Stopped { leaving } => format!("stopped partway — {leaving}"),
        Outcome::Declined => "left alone".to_owned(),
        Outcome::WouldOverwrite => "refused, it would overwrite your own change".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::app::repair::{Beyond, Mended, Report};
    use lemonfiber_core::error::Remedy;
    use lemonfiber_core::repair::{agreement, Outcome, Repair};

    use super::{mended, reversed};
    use lemonfiber_core::journal::{Action, Undo};

    fn repair() -> Repair {
        Repair {
            check: "vpn.port-forward-client".to_owned(),
            does: "Move the download client onto the forwarded port".to_owned(),
            effects: Vec::new(),
            reversible: true,
        }
    }

    /// A fault that has outlasted every repair for it, and where to go instead.
    fn beyond() -> Beyond {
        Beyond {
            check: "vpn.port-forward-client".to_owned(),
            remedy: Remedy::new("Ask for help with a support bundle")
                .with_detail("lemonfiber support --logs 500"),
        }
    }

    fn report(acted: bool, outcomes: Vec<Outcome>) -> Report {
        Report {
            offered: vec![repair()],
            agreement: agreement(&[repair()]),
            beyond: Vec::new(),
            mended: outcomes
                .into_iter()
                .map(|outcome| Mended {
                    repair: repair(),
                    outcome,
                })
                .collect(),
            acted,
        }
    }

    /// A stack with nothing lemonfiber can mend says so, rather than printing an empty
    /// list that reads as a command that did not work.
    #[test]
    fn nothing_to_mend_is_said_rather_than_shown_as_nothing() {
        let text = mended(&Report::default()).text();
        assert!(
            text.contains("nothing here lemonfiber can put right"),
            "{text}"
        );
    }

    /// A run that only looked says what could be done and that it did none of it.
    #[test]
    fn a_run_that_only_looked_says_so() {
        let text = mended(&report(false, Vec::new())).text();
        assert!(text.contains("could be put right"), "{text}");
        assert!(text.contains("Nothing has been changed"), "{text}");
        assert!(text.contains("--fix"), "{text}");
    }

    /// The difference between a repair that worked and one that merely ran is the whole
    /// point of asking the check again, so the report never blurs the two.
    #[test]
    fn every_outcome_reads_as_what_it_was() {
        let text = mended(&report(
            true,
            vec![
                Outcome::Fixed,
                Outcome::FixFailed,
                Outcome::Declined,
                Outcome::WouldOverwrite,
                Outcome::Stopped {
                    leaving: "the client on its old port".to_owned(),
                },
            ],
        ))
        .text();

        assert!(text.contains("fixed —"), "{text}");
        assert!(text.contains("still wrong afterwards"), "{text}");
        assert!(text.contains("left alone"), "{text}");
        assert!(text.contains("would overwrite your own change"), "{text}");
        // The state a stopped repair left behind travels with the verdict: it is the most
        // useful sentence in a failure, and flattening it loses what to do next.
        assert!(
            text.contains("stopped partway — the client on its old port"),
            "{text}"
        );
    }

    /// The thing an operator has to act on themselves, said first and never left implicit:
    /// a repair that quietly stopped being offered leaves them watching a fault nobody
    /// mentions any more, which is worse than being told it is past what lemonfiber knows.
    #[test]
    fn a_fault_past_repairing_is_named_with_somewhere_to_go() {
        let past = Report {
            offered: Vec::new(),
            agreement: agreement(&[]),
            mended: Vec::new(),
            beyond: vec![beyond()],
            acted: false,
        };
        let text = mended(&past).text();

        assert!(text.contains("has outlasted every repair"), "{text}");
        assert!(text.contains("support bundle"), "{text}");
        assert!(text.contains("lemonfiber support --logs 500"), "{text}");
        // Nothing was offerable, but something was said — so the "nothing to put right"
        // line would be untrue here and is not printed.
        assert!(
            !text.contains("nothing here lemonfiber can put right"),
            "{text}"
        );
    }

    /// Every kind of reversal reads in the words of the thing it acted on, because "undone"
    /// on its own does not tell an operator what their stack now holds.
    #[test]
    fn what_was_put_back_is_said_in_the_terms_of_what_it_changed() {
        let undos = vec![
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Restore {
                    key: "PORT".to_owned(),
                    value: Some("8080".to_owned()),
                    wrote: "6881".to_owned(),
                },
            },
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Restore {
                    key: "PROXY".to_owned(),
                    value: None,
                    wrote: "on".to_owned(),
                },
            },
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Remove {
                    resource: "downloadclient".to_owned(),
                    id: "3".to_owned(),
                },
            },
            Undo {
                target: "disk".to_owned(),
                action: Action::Delete {
                    path: "/tmp/lemonfiber-scratch".to_owned(),
                },
            },
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Reconfigure {
                    resource: "downloadclient".to_owned(),
                    id: "7".to_owned(),
                    field: "tvCategory".to_owned(),
                    value: Some("old-sonarr".to_owned()),
                },
            },
            Undo {
                target: "sonarr".to_owned(),
                action: Action::Reconfigure {
                    resource: "downloadclient".to_owned(),
                    id: "8".to_owned(),
                    field: "movieCategory".to_owned(),
                    value: None,
                },
            },
        ];

        let said = reversed(&undos).text();

        assert!(said.contains("PORT back to 8080"), "{said}");
        assert!(said.contains("PROXY removed, as it was"), "{said}");
        assert!(said.contains("downloadclient 3 removed"), "{said}");
        assert!(said.contains("/tmp/lemonfiber-scratch removed"), "{said}");
        assert!(
            said.contains("downloadclient's tvCategory back to old-sonarr"),
            "{said}"
        );
        assert!(
            said.contains("downloadclient's movieCategory cleared, as it was"),
            "{said}"
        );
    }

    /// A run with nothing to put back says so, rather than showing an empty list that
    /// reads as a command that did not work.
    #[test]
    fn nothing_to_put_back_is_said_plainly() {
        assert!(reversed(&[]).text().contains("no repair"));
    }
}
